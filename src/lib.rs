//! Arch Linux repository parser
//!
//! ## Example
//! ```ignore
//! use archlinux_repo::Repository;
//! async fn main() {
//!     let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
//!         .await
//!         .unwrap();
//!     let gtk = &repo["mingw-w64-gtk3"];
//!     for package in &repo {
//!         println!("{}", &package.name);
//!     }
//! }
//! ```
mod data;
#[macro_use]
extern crate lazy_static;
use data::PackageFiles;
pub use data::{
    Dependency, DependencyConstraints, DependencyConstraintsParseError, DependencyVersion,
    DependencyVersionParseError, Package,
};
use flate2::read::GzDecoder;
use reqwest::{StatusCode, Url};
use serde::__private::Formatter;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;
use std::io::{Cursor, Read, Write};
use std::ops::Index;
use std::sync::Arc;
use tar::Archive;

#[derive(Clone, Debug, PartialEq)]
pub struct HttpError {
    status: StatusCode,
}

impl Display for HttpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "Server returned {} status", self.status.as_u16())
    }
}

impl std::error::Error for HttpError {}

/// Loading progress
pub enum Progress {
    /// Sending request to db file
    LoadingDb,
    /// Reading response chunks of db file. Parameters are: bytes read, file size if present
    LoadingDbChunk(u64, Option<u64>),
    /// Reading database file from archive. Parameter is file name
    ReadingDbFile(String),
    /// Database loaded
    ReadingDbDone,
    /// Sending request to files metadata file
    LoadingFilesMetadata,
    /// Reading response chunk of files metadata file. Parameters are: bytes read, file size if present
    LoadingFilesMetadataChunk(u64, Option<u64>),
    /// Reading files metadata file from archive. Parameter is file name
    ReadingFilesMetadataFile(String),
    /// Files metadata loaded
    ReadingFilesDone,
}

impl Display for Progress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Progress::LoadingDb => write!(f, "Loading repository database"),
            Progress::LoadingDbChunk(current, max) => {
                if let Some(m) = max {
                    write!(f, "Loading repository: {} of {} bytes", current, m)
                } else {
                    write!(f, "Loading repository: {} bytes", current)
                }
            }
            Progress::ReadingDbFile(name) => write!(f, "Loading repository file: {}", name),
            Progress::LoadingFilesMetadata => write!(f, "Loading files metadata"),
            Progress::LoadingFilesMetadataChunk(current, max) => {
                if let Some(m) = max {
                    write!(f, "Loading files metadata: {} of {} bytes", current, m)
                } else {
                    write!(f, "Loading files metadata: {} bytes", current)
                }
            }
            Progress::ReadingFilesMetadataFile(name) => {
                write!(f, "Loading files metadata file: {}", name)
            }
            Progress::ReadingDbDone => write!(f, "Database loaded"),
            Progress::ReadingFilesDone => write!(f, "Files metadata loaded"),
        }
    }
}

lazy_static! {
    static ref SUFFIXES: Vec<&'static str> = vec!["-cvs", "-svn", "-hg", "-darcs", "-bzr", "-git"];
}

#[derive(Default)]
struct Inner {
    packages: Vec<Arc<Package>>,
    package_base: HashMap<String, Arc<Package>>,
    package_name: HashMap<String, Arc<Package>>,
    package_version: HashMap<String, Arc<Package>>,
    package_files: HashMap<String, PackageFiles>,
}

impl Inner {
    async fn load<P>(
        url: &str,
        name: &str,
        load_files_meta: bool,
        progress: P,
    ) -> Result<Self, Box<dyn Error>>
    where
        P: Fn(Progress),
    {
        let mut inner = Inner::default();
        inner.load_db(url, name, &progress).await?;
        if load_files_meta {
            inner.load_files(url, name, &progress).await?;
        }
        Ok(inner)
    }

    async fn load_db<P>(&mut self, url: &str, name: &str, progress: P) -> Result<(), Box<dyn Error>>
    where
        P: Fn(Progress),
    {
        let db_url = format!("{}/{}.db.tar.gz", url, name);
        progress(Progress::LoadingDb);
        let mut db_archive =
            Inner::load_archive(&db_url, |r, a| progress(Progress::LoadingDbChunk(r, a))).await?;
        for entry_result in db_archive.entries()? {
            let mut entry = entry_result?;
            let path = entry.path()?.to_str().unwrap().to_owned();
            if path.ends_with("/desc") {
                progress(Progress::ReadingDbFile(path));
                let mut contents = String::new();
                entry.read_to_string(&mut contents)?;
                let package: Package = archlinux_repo_parser::from_str(&contents)?;
                self.insert(package);
            }
        }
        progress(Progress::ReadingDbDone);
        Ok(())
    }

    async fn load_files<P>(
        &mut self,
        url: &str,
        name: &str,
        progress: P,
    ) -> Result<(), Box<dyn Error>>
    where
        P: Fn(Progress),
    {
        let db_url = format!("{}/{}.files.tar.gz", url, name);
        progress(Progress::LoadingFilesMetadata);
        let mut db_archive = Inner::load_archive(&db_url, |r, a| {
            progress(Progress::LoadingFilesMetadataChunk(r, a))
        })
        .await?;
        for entry_result in db_archive.entries()? {
            let mut entry = entry_result?;
            let path = entry.path()?.to_str().unwrap().to_owned();
            if path.ends_with("/files") {
                progress(Progress::ReadingFilesMetadataFile(path.clone()));
                let mut contents = String::new();
                entry.read_to_string(&mut contents)?;
                let files: PackageFiles = archlinux_repo_parser::from_str(&contents)?;
                let name = path.replace("/files", "").replace("/", "");
                let package = &self.package_version[&name];
                self.package_files.insert(package.name.to_owned(), files);
            }
        }
        progress(Progress::ReadingFilesDone);
        Ok(())
    }

    fn insert(&mut self, package: Package) {
        let package_ref = self.insert_into_maps(package);
        for suffix in SUFFIXES.iter() {
            if package_ref.name.ends_with(suffix) {
                let base_name = package_ref.name.replace(suffix, "");
                let mut base_package = self
                    .package_name
                    .get(&base_name)
                    .map(|p| p.as_ref().clone())
                    .unwrap_or_else(|| Package::base_package_for_csv(package_ref.as_ref(), suffix));
                base_package.linked_sources.push(package_ref.clone());
                self.insert_into_maps(base_package);
            }
        }
    }

    fn insert_into_maps(&mut self, package: Package) -> Arc<Package> {
        let package_ref = Arc::new(package);
        if let Some(base) = package_ref.base.as_ref() {
            if let std::collections::hash_map::Entry::Vacant(e) =
                self.package_base.entry(base.to_owned())
            {
                e.insert(package_ref.clone());
            } else {
                log::warn!("[archlinux-repo-rs] Found package {} with already registered base name! Ignoring...", &package_ref.name)
            }
        }
        self.package_name
            .insert(package_ref.name.to_owned(), package_ref.clone());
        self.package_version.insert(
            package_ref.name.to_owned() + "-" + &package_ref.version,
            package_ref.clone(),
        );
        self.packages.push(package_ref.clone());
        package_ref
    }

    async fn load_archive<P>(
        url: &str,
        progress: P,
    ) -> Result<Archive<Cursor<Vec<u8>>>, Box<dyn Error>>
    where
        P: Fn(u64, Option<u64>),
    {
        let mut enc_buf = Vec::new();
        let mut response = reqwest::get(Url::parse(&url)?).await?;
        if !response.status().is_success() {
            return Err(Box::new(HttpError {
                status: response.status(),
            }));
        }
        let mut bytes_read: u64 = 0;
        let length = response.content_length();
        while let Some(chunk) = response.chunk().await? {
            enc_buf.write_all(&chunk[..])?;
            bytes_read += chunk.len() as u64;
            progress(bytes_read, length);
        }
        let mut decoder = GzDecoder::new(&enc_buf[..]);
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf)?;
        Ok(Archive::new(Cursor::new(buf)))
    }
}

/// Arch Linux repository
pub struct Repository {
    inner: Inner,
    url: String,
    name: String,
    load_files_meta: bool,
    progress_listener: Option<Box<dyn Fn(Progress)>>,
}

impl Repository {
    async fn new(
        url: String,
        name: String,
        load_files_meta: bool,
        progress_listener: Option<Box<dyn Fn(Progress)>>,
    ) -> Result<Self, Box<dyn Error>> {
        let listener = progress_listener.as_ref();
        let inner = Inner::load(&url, &name, load_files_meta, |progress| {
            if let Some(l) = listener {
                l(progress)
            }
        })
        .await?;
        Ok(Repository {
            progress_listener,
            load_files_meta,
            name,
            url,
            inner,
        })
    }
    /// Loads arch repository by it's name and url
    ///
    /// # Example
    /// ```ignore
    /// use archlinux_repo::Repository;
    ///
    /// let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64").await?;
    /// ```
    pub async fn load(name: &str, url: &str) -> Result<Repository, Box<dyn Error>> {
        RepositoryBuilder::new(name, url).load().await
    }

    /// Get package by full name. Will return `None` if package cannot be found
    ///
    /// # Example
    /// ```ignore
    /// use archlinux_repo::Repository;
    ///
    /// let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64").await?;
    /// let gtk = repo.get_package_by_name("mingw-w64-x86_64-gtk3")?;
    /// ```
    pub fn get_package_by_name(&self, name: &str) -> Option<&Package> {
        self.inner.package_name.get(name).map(|p| p as &Package)
    }

    /// Get package by full name and version. Will return `None` if package cannot be found
    ///
    /// # Example
    /// ```ignore
    /// use archlinux_repo::Repository;
    ///
    /// let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64").await?;
    /// let gtk = repo.get_package_by_name_and_version("mingw-w64-x86_64-gtk3-3.24.9-4")?;
    /// ```
    pub fn get_package_by_name_and_version(&self, name: &str) -> Option<&Package> {
        self.inner.package_version.get(name).map(|p| p as &Package)
    }

    /// Get package by base name. Will return `None` if package cannot be found
    ///
    /// **NOTE! Not all packages have names**
    ///
    /// # Example
    /// ```ignore
    /// use archlinux_repo::Repository;
    ///
    /// let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64").await?;
    /// let gtk = repo.get_package_by_base("mingw-w64-gtk3")?;
    /// ```
    pub fn get_package_by_base(&self, name: &str) -> Option<&Package> {
        self.inner.package_base.get(name).map(|p| p as &Package)
    }

    /// Get package files by full name.
    /// Will return `None` if package cannot be found or does not contains file metadata
    ///
    /// **NOTE! This method will always return None if `load_files_meta` is `false`**
    /// **NOTE! For CSV packages base package name will always return None unless it exists in repo**
    ///
    /// # Example
    /// ```ignore
    /// use archlinux_repo::{Repository, RepositoryBuilder};
    ///
    /// let repo = RepositoryBuilder::new("mingw64", "http://repo.msys2.org/mingw/x86_64")
    ///                 .files_metadata(true)
    ///                 .load()
    ///                 .await?;
    /// let gtk_files = repo.get_package_files("mingw-w64-x86_64-gtk3")?;
    /// ```
    pub fn get_package_files(&self, name: &str) -> Option<&Vec<String>> {
        self.inner.package_files.get(name).map(|m| &m.files)
    }

    /// Send HTTP request to download package by full name/base name or name with version.
    /// Panics if package not found
    ///
    /// # Example
    /// ```ignore
    /// use archlinux_repo::Repository;
    ///
    /// let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64").await?;
    /// let gtk_package = repo.request_package("mingw-w64-gtk3").await?.bytes().await?;
    /// ```
    pub async fn request_package(&self, name: &str) -> Result<reqwest::Response, Box<dyn Error>> {
        let package = self.index(name);
        let url = format!("{}/{}", self.url, package.file_name);
        Ok(reqwest::get(Url::parse(&url)?).await?)
    }

    /// Reload repository
    //TODO signature verification
    pub async fn reload(&mut self) -> Result<(), Box<dyn Error>> {
        let listener = self.progress_listener.as_ref();
        self.inner = Inner::load(&self.url, &self.name, self.load_files_meta, |progress| {
            if let Some(l) = listener {
                l(progress)
            }
        })
        .await?;
        Ok(())
    }
}

impl Index<&str> for Repository {
    type Output = Package;

    #[inline]
    fn index(&self, index: &str) -> &Self::Output {
        self.get_package_by_base(index)
            .or_else(|| self.get_package_by_name(index))
            .or_else(|| self.get_package_by_name_and_version(index))
            .expect("package not found")
    }
}

impl<'a> IntoIterator for &'a Repository {
    type Item = &'a Package;
    type IntoIter = Box<(dyn Iterator<Item = Self::Item> + 'a)>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.inner.packages.iter().map(|v| &**v))
    }
}

/// Repository builder
///
/// # Example
/// ```ignore
/// use archlinux_repo::RepositoryBuilder;;
///
/// RepositoryBuilder::new("mingw64", "http://repo.msys2.org/mingw/x86_64")
///                         .files_metadata(true)
///                         .progress_listener(Box::new(|p| println!("{}", p)))
///                         .load()
///                         .await?;
/// ```
pub struct RepositoryBuilder {
    name: String,
    url: String,
    files_meta: bool,
    progress_listener: Option<Box<dyn Fn(Progress)>>,
}

impl RepositoryBuilder {
    /// Create new repository builder with repository name and url
    pub fn new(name: &str, url: &str) -> Self {
        RepositoryBuilder {
            name: name.to_owned(),
            url: url.to_owned(),
            files_meta: false,
            progress_listener: None,
        }
    }

    /// Enable or disable loading files metadata
    pub fn files_metadata(mut self, load: bool) -> Self {
        self.files_meta = load;
        self
    }

    /// Set load progress listener
    pub fn progress_listener(mut self, listener: Box<dyn Fn(Progress)>) -> Self {
        self.progress_listener = Some(listener);
        self
    }

    /// Create and load repository
    pub async fn load(self) -> Result<Repository, Box<dyn Error>> {
        Ok(Repository::new(self.url, self.name, self.files_meta, self.progress_listener).await?)
    }
}

#[cfg(test)]
mod test {
    use crate::data::PackageFiles;
    use crate::{Package, Repository, RepositoryBuilder};

    #[tokio::test]
    async fn repo_loads_msys2_mingw_repo() {
        Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn get_gtk_by_name() {
        let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        let gtk = repo.get_package_by_name("mingw-w64-x86_64-gtk3").unwrap();
        assert_eq!("mingw-w64-gtk3", gtk.base.as_ref().unwrap())
    }

    #[tokio::test]
    async fn get_none_from_not_existing_name() {
        let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        let package = repo.get_package_by_name("not_exist");
        assert!(package.is_none())
    }

    #[tokio::test]
    async fn get_gtk_by_base() {
        let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        let gtk = repo.get_package_by_base("mingw-w64-gtk3").unwrap();
        assert_eq!("mingw-w64-x86_64-gtk3", &gtk.name)
    }

    #[tokio::test]
    async fn get_none_from_not_existing_base() {
        let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        let package = repo.get_package_by_base("not_exist");
        assert!(package.is_none());
    }

    #[tokio::test]
    async fn get_gtk_by_name_and_version() {
        let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        let gtk = repo.get_package_by_name("mingw-w64-x86_64-gtk3").unwrap();
        assert_eq!("mingw-w64-gtk3", gtk.base.as_ref().unwrap());
        let gtk_name_and_version = format!("mingw-w64-x86_64-gtk3-{}", &gtk.version);
        let gtk_from_ver = repo
            .get_package_by_name_and_version(&gtk_name_and_version)
            .unwrap();
        assert_eq!(gtk, gtk_from_ver);
    }

    #[tokio::test]
    async fn get_none_from_not_existing_name_and_version() {
        let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        let package = repo.get_package_by_name_and_version("not_exist-1.0.0");
        assert!(package.is_none());
    }

    #[tokio::test]
    async fn get_gtk_files_with_file_metadata_enabled() {
        let repo = RepositoryBuilder::new("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .files_metadata(true)
            .load()
            .await
            .unwrap();
        assert!(!repo
            .get_package_files("mingw-w64-x86_64-gtk3")
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn get_none_with_file_metadata_disabled() {
        let repo = RepositoryBuilder::new("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .files_metadata(false)
            .load()
            .await
            .unwrap();
        assert!(repo.get_package_files("mingw-w64-x86_64-gtk3").is_none());
    }

    #[tokio::test]
    async fn get_none_with_default() {
        let repo = RepositoryBuilder::new("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .load()
            .await
            .unwrap();
        assert!(repo.get_package_files("mingw-w64-x86_64-gtk3").is_none());
    }

    #[tokio::test]
    async fn get_gtk_by_index_and_full_name() {
        let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        let gtk = &repo["mingw-w64-x86_64-gtk3"];
        assert_eq!("mingw-w64-gtk3", gtk.base.as_ref().unwrap());
    }

    #[tokio::test]
    async fn get_libwinpthread_by_csv_and_base_names() {
        let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        let a = repo
            .get_package_by_name("mingw-w64-x86_64-libwinpthread-git")
            .unwrap();
        let b = repo
            .get_package_by_name("mingw-w64-x86_64-libwinpthread")
            .unwrap();
        assert_eq!(1, b.linked_sources.len());
        assert_eq!(a, b.linked_sources[0].as_ref())
    }

    #[tokio::test]
    async fn get_gtk_by_index_and_base_name() {
        let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        let gtk = &repo["mingw-w64-gtk3"];
        assert_eq!("mingw-w64-x86_64-gtk3", &gtk.name);
    }

    #[tokio::test]
    async fn get_gtk_by_index_and_full_name_and_version() {
        let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        let gtk = repo.get_package_by_name("mingw-w64-x86_64-gtk3").unwrap();
        assert_eq!("mingw-w64-gtk3", gtk.base.as_ref().unwrap());
        let gtk_name_and_version = format!("mingw-w64-x86_64-gtk3-{}", &gtk.version);
        let gtk_package = &repo[&gtk_name_and_version];
        assert_eq!("mingw-w64-x86_64-gtk3", &gtk_package.name);
    }

    #[tokio::test]
    async fn request_gtk_by_full_name() {
        let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        let bytes = repo
            .request_package("mingw-w64-x86_64-gtk3")
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert!(!&bytes[..].is_empty());
    }

    #[tokio::test]
    async fn request_gtk_by_full_name_and_version() {
        let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        let gtk = repo.get_package_by_name("mingw-w64-x86_64-gtk3").unwrap();
        assert_eq!("mingw-w64-gtk3", gtk.base.as_ref().unwrap());
        let gtk_name_and_version = format!("mingw-w64-x86_64-gtk3-{}", &gtk.version);
        let bytes = repo
            .request_package(&gtk_name_and_version)
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert!(!&bytes[..].is_empty());
    }

    #[tokio::test]
    async fn request_gtk_by_base_name() {
        let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        let bytes = repo
            .request_package("mingw-w64-gtk3")
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert!(!&bytes[..].is_empty());
    }

    #[tokio::test]
    async fn iterator_should_have_gtk() {
        let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        for package in &repo {
            if package.name == "mingw-w64-x86_64-gtk3" {
                return;
            }
        }
        unreachable!();
    }

    #[tokio::test]
    async fn reload_should_not_fail() {
        let mut repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
        repo.reload().await.unwrap();
    }

    #[tokio::test]
    async fn should_report_progress() {
        RepositoryBuilder::new("mingw64", "http://repo.msys2.org/mingw/x86_64")
            .files_metadata(true)
            .progress_listener(Box::new(|p| println!("{}", p)))
            .load()
            .await
            .unwrap();
    }

    #[tokio::test]
    #[should_panic]
    async fn should_not_load_bad_repo() {
        Repository::load("bad", "http://repo.msys2.org/mingw/x86_64")
            .await
            .unwrap();
    }

    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Package>();
        assert_send::<PackageFiles>();
    }

    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Package>();
        assert_sync::<PackageFiles>();
    }
}
