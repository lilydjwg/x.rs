use std::io::Result as IoResult;
use std::io::ErrorKind;
use std::process::{Command, ExitStatus};
use std::path::{Path, PathBuf};
use std::fs::{
  DirBuilder, read_dir, remove_dir, rename,
  remove_file,
};
use std::ffi::OsStr;
use std::os::unix::process::ExitStatusExt;
use std::os::unix::ffi::OsStrExt;
use std::fmt::Write;
use std::env;

fn main() {
  for arg in env::args_os().skip(1) {
    extract(arg);
  }
}

fn extract<P: AsRef<Path>>(f: P) {
  let f = f.as_ref();

  if f.extension() == Some(OsStr::new("deb")) {
    return extract_deb(f);
  }

  let cmd = match get_cmd_for_file(f) {
    None => {
      eprintln!(
        "no idea to extract file: {}", f.display());
      std::process::exit(21);
    },
    Some(cmd) => cmd,
  };

  let topdir = derive_dir_path(f);
  create_target_path(&topdir);

  let st = Command::new(cmd[0])
    .args(&cmd[1..])
    .arg(Path::new("..").join(f))
    .current_dir(&topdir)
    .status()
    .expect("failed to execute extractor");

  check_exit_status(st);

  let mut it = read_dir(&topdir).unwrap().into_iter();
  let first = it.next().unwrap();
  if let None = it.next() {
    move_up(&topdir, &first.unwrap().path()).unwrap();
  }
}

fn check_exit_status(st: ExitStatus) {
  if !st.success() {
    std::process::exit(
      st.code().unwrap_or_else(
        || st.signal().unwrap() + 128
      )
    );
  }
}

fn move_up<D: AsRef<Path>, U: AsRef<Path>>(
  topdir: D, under: U,
) -> IoResult<()> {
  let topdir = topdir.as_ref();
  let basename_os = under.as_ref().file_name().unwrap();
  let mut basename;
  let orig_updir = Path::new(basename_os);
  let mut updir = orig_updir;

  if updir.exists() {
    if updir != topdir {
      basename = basename_os.to_string_lossy().into_owned();
      let len = basename.len();
      let mut i = 1;
      updir = loop {
        write!(basename, "{}", i).unwrap();
        let dir = Path::new(&basename);
        if !dir.exists() {
          break dir;
        }
        basename.truncate(len);
        i += 1;
        if i == 100 {
          break orig_updir;
        }
      };
    } else {
      let tempname = format!("{}.tmp.{}",
        topdir.file_name().unwrap().to_string_lossy(),
        unsafe { libc::getpid() });
      let tempdir = topdir.with_file_name(tempname);
      // move out
      rename(under.as_ref(), &tempdir)?;
      // remove old
      remove_dir(topdir)?;
      // rename to new
      return rename(tempdir, &topdir)
    }
  }

  rename(under.as_ref(), &updir)?;
  remove_dir(topdir)
}

fn derive_dir_path(p: &Path) -> PathBuf {
  if let Some(stem) = p.file_stem() {
    let mut stem_b = stem.as_bytes();
    if stem_b.ends_with(b".tar") {
      stem_b = &stem_b[..stem_b.len()-4];
      if stem_b.ends_with(b".pkg") {
        stem_b = &stem_b[..stem_b.len()-4];
      }
    }
    let mut ret = PathBuf::new();
    ret.push(OsStr::from_bytes(stem_b));
    ret
  } else {
    panic!("can't derive a directory name from provided file path")
  }
}

fn create_target_path(p: &Path) {
  if let Err(e) = DirBuilder::new().create(p) {
    if e.kind() == ErrorKind::AlreadyExists &&
      dir_is_empty(&p).unwrap() {
    } else {
      eprintln!(
        "target directory exists and is not empty: {}",
        p.display());
      std::process::exit(22);
    }
  }
}

fn dir_is_empty<P: AsRef<Path>>(dir: P) -> IoResult<bool> {
  for _ in read_dir(dir.as_ref())? {
    return Ok(false);
  }
  Ok(true)
}

static EXTS_TO_CMD: &[(&[&str], &[&str])] = &[
  (&[".tar.gz", ".tar.xz", ".tar.zst", ".tar.bz2", ".tgz", ".txz", ".tbz", ".tar"], &["tar", "xvf"]),
  (&[".7z", ".chm", ".a"], &["7z", "x"]),
  (&[".zip"], &["gbkunzip"]),
  (&[".xpi", ".jar", ".apk", ".maff", ".epub", ".crx", ".whl", ".xapk"], &["unzip"]),
];

fn get_cmd_for_file(f: &Path) -> Option<&[&str]> {
  let file_name = f.file_name().unwrap();
  for (exts, cmd) in EXTS_TO_CMD {
    if exts.iter().any(|x| file_name.as_bytes().ends_with(x.as_bytes())) {
      return Some(cmd);
    }
  }
  if file_name.as_bytes().ends_with(b".rar") {
    let output = Command::new("file")
      .arg(f)
      .output()
      .expect("invoke 'file' command");

    return Some(
      if twoway::find_bytes(&output.stdout, b"Win32").is_some() {
        &["7z", "x"]
      } else {
        &["rar", "x"]
      }
    )
  }
  None
}

fn extract_deb<P: AsRef<Path>>(f: P) {
  let f = f.as_ref();
  let topdir = derive_dir_path(f);
  create_target_path(&topdir);

  let st = Command::new("bsdtar")
    .arg("xf")
    .arg(Path::new("..").join(f))
    .current_dir(&topdir)
    .status()
    .expect("failed to execute extractor bsdtar");

  check_exit_status(st);

  let files = read_dir(&topdir).unwrap()
    .collect::<IoResult<Vec<_>>>().unwrap();

  env::set_current_dir(&topdir).unwrap();
  for f in files {
    if !&["debian-binary", "_gpgorigin"].iter().any(|x| x == &f.file_name()) {
      extract(f.file_name());
      remove_file(f.file_name()).unwrap();
    }
  }
}
