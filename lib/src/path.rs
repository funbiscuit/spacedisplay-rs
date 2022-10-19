use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use crc::CRC_16_ISO_IEC_14443_3_A;

// using 16bit to reduce memory usage. Changing to u32 doesn't speed up
// scanning but memory usage grows ~32Mb per 500k entries
pub type PathCrc = u16;

const CRC_BUILDER: crc::Crc<PathCrc> = crc::Crc::<PathCrc>::new(&CRC_16_ISO_IEC_14443_3_A);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryPath {
    parts: Vec<String>,
}

impl EntryPath {
    /// Adds new path part to the end of the path
    pub fn join(&mut self, part: String) {
        self.parts.push(part);
    }

    /// Calculate crc given parts of path
    ///
    /// Crc is XOR of crc of individual parts
    /// Returns `None` if given slice is empty
    pub fn calc_crc<T: AsRef<str>>(parts: &[T]) -> Option<PathCrc> {
        parts
            .iter()
            .map(|p| CRC_BUILDER.checksum(p.as_ref().as_bytes()))
            .reduce(|accum, item| accum ^ item)
    }

    /// Calculate crc that represents this path
    pub fn get_crc(&self) -> PathCrc {
        // parts is never empty, CRC is calculated over all parts
        //todo store path crc and return already calculated value
        EntryPath::calc_crc(&self.parts).unwrap()
    }

    /// Get filename of this path
    pub fn get_name(&self) -> &str {
        self.parts.last().unwrap()
    }

    /// Get `PathBuf` representing this path
    pub fn get_path(&self) -> PathBuf {
        // parts is never empty
        let mut path = PathBuf::from(&self.parts[0]);

        for part in &self.parts[1..] {
            path = path.join(part);
        }

        path
    }

    /// Change path to its parent
    pub fn go_up(&mut self) {
        assert!(self.parts.len() > 1);
        self.parts.pop();
    }

    /// Returns `true` if this path is a root path
    pub fn is_root(&self) -> bool {
        self.parts.len() == 1
    }

    /// Create new entry path from `Path` and root
    ///
    /// Returns `None` if path doesn't start from root or if it contains non unicode characters
    /// and can't be represented as String
    pub fn from<P1: AsRef<Path>, P2: AsRef<Path>>(root: P1, path: P2) -> Option<Self> {
        let child_path = path.as_ref().strip_prefix(root.as_ref()).ok()?;

        let parts = std::iter::once(root.as_ref().as_os_str())
            .chain(child_path.iter())
            .map(|s| s.to_str().map(|s| s.to_string()))
            .collect::<Option<Vec<_>>>()?;

        Some(EntryPath { parts })
    }

    /// Creates new `EntryPath` with root only
    pub fn new(root: String) -> Self {
        EntryPath { parts: vec![root] }
    }

    pub fn parts(&self) -> &[String] {
        &self.parts
    }
}

impl Display for EntryPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // only Strings are stored, so this should not fail
        write!(f, "{}", self.get_path().to_str().unwrap())
    }
}

/// path1 < path2 if path2 contains path1.
/// Example:
/// /data < /data/test
/// /mnt/file > /mnt
/// /mnt == /mnt
///
/// For different paths result is None
/// Example:
/// partial_cmp(/mnt/data, /mnt/test) == None
impl PartialOrd for EntryPath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mut left: &[String] = &self.parts;
        let mut right: &[String] = &other.parts;

        loop {
            if left.is_empty() && right.is_empty() {
                return Some(Ordering::Equal);
            }
            if left.is_empty() {
                return Some(Ordering::Less);
            }
            if right.is_empty() {
                return Some(Ordering::Greater);
            }
            if left[0] != right[0] {
                return None;
            }
            left = &left[1..];
            right = &right[1..];
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::path::{EntryPath, CRC_BUILDER};

    fn path<T: Into<PathBuf>>(name: T) -> PathBuf {
        name.into()
    }

    #[test]
    fn crc() {
        assert_eq!(
            EntryPath::calc_crc(&["part1", "part2"]).unwrap(),
            CRC_BUILDER.checksum("part1".as_bytes()) ^ CRC_BUILDER.checksum("part2".as_bytes())
        );
        assert_eq!(
            EntryPath::from(&path("/data"), &path("/data/dir/test"))
                .unwrap()
                .get_crc(),
            EntryPath::calc_crc(&["/data", "dir", "test"]).unwrap()
        );
    }

    #[test]
    fn from() {
        assert_eq!(
            EntryPath::from(&path("/data/"), &path("/data/test"))
                .unwrap()
                .parts,
            vec!["/data/".to_string(), "test".to_string()]
        );
        assert_eq!(
            EntryPath::from(&path("/data/"), &path("/data"))
                .unwrap()
                .parts,
            vec!["/data/".to_string()]
        );
        assert_eq!(
            EntryPath::from(&path("/data"), &path("/data/file"))
                .unwrap()
                .parts,
            vec!["/data".to_string(), "file".to_string()]
        );
        assert_eq!(
            EntryPath::from(&path("/data/"), &path("/data")).unwrap(),
            EntryPath::new("/data/".to_string())
        );
    }

    #[test]
    fn get_path() {
        assert_eq!(
            EntryPath::from(&path("/data/"), &path("/data/"))
                .unwrap()
                .get_path(),
            PathBuf::from("/data")
        );
        assert_eq!(
            EntryPath::from(&path("/data/"), &path("/data/test"))
                .unwrap()
                .get_path(),
            PathBuf::from("/data/test")
        );
        assert_eq!(
            EntryPath::from(&path("/data/mnt"), &path("/data/mnt/test"))
                .unwrap()
                .get_path(),
            PathBuf::from("/data/mnt/test")
        );
    }

    #[test]
    fn cmp_and_eq() {
        assert_eq!(
            EntryPath::from(&path("/data/"), &path("/data/test")).unwrap(),
            EntryPath::from(&path("/data/"), &path("/data/test")).unwrap()
        );
        assert!(
            EntryPath::from(&path("/data/"), &path("/data/")).unwrap()
                < EntryPath::from(&path("/data/"), &path("/data/test")).unwrap()
        );
        assert!(
            EntryPath::from(&path("/data/"), &path("/data/test")).unwrap()
                > EntryPath::from(&path("/data/"), &path("/data/")).unwrap()
        );
        assert!(EntryPath::from(&path("/data/"), &path("/data/test"))
            .unwrap()
            .partial_cmp(&EntryPath::from(&path("/data/"), &path("/data/other")).unwrap())
            .is_none());
    }
}
