use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use reqwest::{StatusCode, Url};
use serde::export::Formatter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;
use std::io::{Cursor, Read, Write};
use std::ops::Index;
use std::rc::Rc;
use tar::Archive;

/// Repository package
#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, Debug)]
pub struct Package {
    /// file name
    #[serde(rename = "FILENAME")]
    pub file_name: String,
    /// name
    #[serde(rename = "NAME")]
    pub name: String,
    /// name without architecture
    #[serde(rename = "BASE")]
    pub base: Option<String>,
    /// version
    #[serde(rename = "VERSION")]
    pub version: String,
    /// description
    #[serde(rename = "DESC")]
    pub description: Option<String>,
    /// package groups
    #[serde(rename = "GROUPS")]
    pub groups: Option<Vec<String>>,
    /// tar.xz archive size
    #[serde(rename = "CSIZE")]
    pub compressed_size: u64,
    /// installed files size
    #[serde(rename = "ISIZE")]
    pub installed_size: u64,
    /// MD5 checksum
    #[serde(rename = "MD5SUM")]
    pub md5_sum: String,
    /// SHA256 checksum
    #[serde(rename = "SHA256SUM")]
    pub sha256_sum: String,
    /// PGP signature
    #[serde(rename = "PGPSIG")]
    pub pgp_signature: String,
    /// package home url
    #[serde(rename = "URL")]
    pub home_url: Option<String>,
    /// license name
    #[serde(rename = "LICENSE")]
    pub license: Option<Vec<String>>,
    /// processor architecture
    #[serde(rename = "ARCH")]
    pub architecture: String,
    /// build date
    #[serde(rename = "BUILDDATE", with = "date_serde")]
    pub build_date: DateTime<Utc>,
    /// who created this package
    #[serde(rename = "PACKAGER")]
    pub packager: String,
    /// packages which this package replaces
    #[serde(rename = "REPLACES")]
    pub replaces: Option<Vec<String>>,
    /// packages which cannot be used with this package
    #[serde(rename = "CONFLICTS")]
    pub conflicts: Option<Vec<String>>,
    /// packages provided by this package
    #[serde(rename = "PROVIDES")]
    pub provides: Option<Vec<String>>,
    /// run-time dependencies
    #[serde(rename = "DEPENDS")]
    pub depends: Option<Vec<String>>,
    #[serde(rename = "OPTDEPENDS")]
    pub optdepends: Option<Vec<String>>,
    /// build-time dependencies
    #[serde(rename = "MAKEDEPENDS")]
    pub makedepends: Option<Vec<String>>,
    #[serde(rename = "CHECKDEPENDS")]
    pub checkdepends: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, Debug)]
struct PackageFiles {
    #[serde(rename = "FILES")]
    files: Vec<String>,
}

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

/// Arch Linux repository
pub struct Repository {
    url: String,
    name: String,
    load_files_meta: bool,
    progress_listener: Option<Box<dyn Fn(Progress) -> ()>>,
    packages: Vec<Rc<Package>>,
    package_base: HashMap<String, Rc<Package>>,
    package_name: HashMap<String, Rc<Package>>,
    package_version: HashMap<String, Rc<Package>>,
    package_files: HashMap<String, PackageFiles>,
}

impl Repository {
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
        self.package_name.get(name).map(|p| p as &Package)
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
        self.package_version.get(name).map(|p| p as &Package)
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
        self.package_base.get(name).map(|p| p as &Package)
    }

    /// Get package files by full name.
    /// Will return `None` if package cannot be found or does not contains file metadata
    ///
    /// **NOTE! This method will always return None if `load_files_meta` is `false`**
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
        self.package_files.get(name).map(|m| &m.files)
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
        self.package_name.clear();
        self.package_base.clear();
        self.package_version.clear();
        self.packages.clear();
        self.package_files.clear();
        self.load_db().await?;
        if self.load_files_meta {
            self.load_files().await?;
        }
        Ok(())
    }

    async fn load_files(&mut self) -> Result<(), Box<dyn Error>> {
        let db_url = format!("{}/{}.files.tar.gz", self.url, self.name);
        self.progress_changed(Progress::LoadingFilesMetadata);
        let mut db_archive = Repository::load_archive(&db_url, |r, a| {
            self.progress_changed(Progress::LoadingFilesMetadataChunk(r, a))
        })
        .await?;
        for entry_result in db_archive.entries()? {
            let mut entry = entry_result?;
            let path = entry.path()?.to_str().unwrap().to_owned();
            if path.ends_with("/files") {
                self.progress_changed(Progress::ReadingFilesMetadataFile(path.clone()));
                let mut contents = String::new();
                entry.read_to_string(&mut contents)?;
                let files: PackageFiles = archlinux_repo_parser::from_str(&contents)?;
                let name = path.replace("/files", "").replace("/", "");
                let package = &self.package_version[&name];
                self.package_files.insert(package.name.to_owned(), files);
            }
        }
        self.progress_changed(Progress::ReadingFilesDone);
        Ok(())
    }

    async fn load_db(&mut self) -> Result<(), Box<dyn Error>> {
        let db_url = format!("{}/{}.db.tar.gz", self.url, self.name);
        self.progress_changed(Progress::LoadingDb);
        let mut db_archive = Repository::load_archive(&db_url, |r, a| {
            self.progress_changed(Progress::LoadingDbChunk(r, a))
        })
        .await?;
        for entry_result in db_archive.entries()? {
            let mut entry = entry_result?;
            let path = entry.path()?.to_str().unwrap().to_owned();
            if path.ends_with("/desc") {
                self.progress_changed(Progress::ReadingDbFile(path));
                let mut contents = String::new();
                entry.read_to_string(&mut contents)?;
                let package: Package = archlinux_repo_parser::from_str(&contents)?;
                let package_ref = Rc::new(package);
                if let Some(base) = package_ref.base.as_ref() {
                    self.package_base
                        .insert(base.to_owned(), package_ref.clone());
                }
                self.package_name
                    .insert(package_ref.name.to_owned(), package_ref.clone());
                self.package_version.insert(
                    (package_ref.name.to_owned() + "-" + &package_ref.version).to_owned(),
                    package_ref.clone(),
                );
                self.packages.push(package_ref);
            }
        }
        self.progress_changed(Progress::ReadingDbDone);
        Ok(())
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

    fn progress_changed(&self, progress: Progress) {
        if let Some(listener) = self.progress_listener.as_ref() {
            listener(progress)
        }
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
        Box::new(self.packages.iter().map(|v| &**v))
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
    progress_listener: Option<Box<dyn Fn(Progress) -> ()>>,
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
    pub fn progress_listener(mut self, listener: Box<dyn Fn(Progress) -> ()>) -> Self {
        self.progress_listener = Some(listener);
        self
    }

    /// Create and load repository
    pub async fn load(self) -> Result<Repository, Box<dyn Error>> {
        let mut repo = Repository {
            url: self.url,
            name: self.name,
            packages: Vec::new(),
            package_version: HashMap::new(),
            package_name: HashMap::new(),
            package_base: HashMap::new(),
            progress_listener: self.progress_listener,
            load_files_meta: self.files_meta,
            package_files: HashMap::new(),
        };
        repo.reload().await?;
        Ok(repo)
    }
}

mod date_serde {
    use chrono::{DateTime, TimeZone, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(date.timestamp())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let timestamp = i64::deserialize(deserializer)?;
        Ok(Utc.timestamp(timestamp, 0))
    }
}

#[cfg(test)]
mod test {
    use crate::{Package, PackageFiles, Repository, RepositoryBuilder};

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
