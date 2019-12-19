use std::io::Result as IoResult;
use std::io::ErrorKind;
use std::process::Command;
use std::path::{Path, PathBuf};
use std::fs::{DirBuilder, read_dir, remove_dir, rename};
use std::ffi::OsStr;
use std::os::unix::process::ExitStatusExt;
use std::os::unix::ffi::OsStrExt;

fn main() {
  for arg in std::env::args_os().skip(1) {
    extract(arg);
  }
}

fn extract<P: AsRef<Path>>(f: P) {
  let f = f.as_ref();
  let cmd = match get_cmd_for_file(f) {
    None => {
      eprintln!(
        "no idea to extract file: {}", f.display());
      std::process::exit(21);
    },
    Some(cmd) => cmd,
  };

  let topdir = derive_dir_path(f);
  if let Err(e) = DirBuilder::new().create(&topdir) {
    if e.kind() == ErrorKind::AlreadyExists &&
      dir_is_empty(&topdir).unwrap() {
    } else {
      eprintln!(
        "target directory exists and is not empty: {}",
        topdir.display());
      std::process::exit(22);
    }
  }

  let st = Command::new(cmd[0])
    .args(&cmd[1..])
    .arg(f)
    .current_dir(&topdir)
    .status()
    .expect("failed to execute extractor");

  if !st.success() {
    std::process::exit(
      st.code().unwrap_or(
        st.signal().unwrap() + 128
      )
    );
  }

  let mut it = read_dir(&topdir).unwrap().into_iter();
  let first = it.next().unwrap();
  if let None = it.next() {
    move_up(&topdir, &first.unwrap().path()).unwrap();
  }
}

fn move_up<D: AsRef<Path>, U: AsRef<Path>>(
  dest: D, under: U,
) -> IoResult<()> {
  let tmpdir = under.as_ref().with_file_name(".tmp.dir__");
  rename(under, &tmpdir)?;

  for entry in read_dir(&tmpdir)? {
    let mut target = dest.as_ref().to_path_buf();
    let src = entry?.path();
    target.push(src.file_name().unwrap());
    rename(&src, &target)?;
  }
  remove_dir(&tmpdir)
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

fn dir_is_empty<P: AsRef<Path>>(dir: P) -> IoResult<bool> {
  for _ in read_dir(dir.as_ref())? {
    return Ok(false);
  }
  Ok(true)
}

static EXTS_TO_CMD: &[(&[&str], &[&str])] = &[
  (&[".tar.gz", ".tar.xz", ".tar.bz2", ".tgz", ".txz", ".tbz", ".tar"], &["tar", "xvf"]),
  (&[".7z", ".chm"], &["7z", "x"]),
  (&[".zip"], &["gbkunzip"]),
  (&[".xpi", ".jar", ".apk", ".maff", ".epub", ".crx", ".whl"], &["unzip"]),
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
