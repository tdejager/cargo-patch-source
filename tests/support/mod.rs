use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use toml_edit::{self, Item, Table};

#[derive(Clone, Debug)]
pub struct DependencySpec {
    version: Option<String>,
    path: Option<String>,
    git: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    rev: Option<String>,
}

impl DependencySpec {
    pub fn version(version: impl Into<String>) -> Self {
        Self {
            version: Some(version.into()),
            path: None,
            git: None,
            branch: None,
            tag: None,
            rev: None,
        }
    }

    pub fn git(url: impl Into<String>) -> Self {
        Self {
            version: None,
            path: None,
            git: Some(url.into()),
            branch: None,
            tag: None,
            rev: None,
        }
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    fn to_item(&self) -> Item {
        let complex = self.path.is_some()
            || self.git.is_some()
            || self.branch.is_some()
            || self.tag.is_some()
            || self.rev.is_some()
            || self
                .version
                .as_ref()
                .map_or(false, |v| v.contains('*') || v.contains('?'));

        if !complex {
            if let Some(version) = &self.version {
                return toml_edit::value(version.clone()).into();
            }
        }

        let mut table = toml_edit::InlineTable::new();
        if let Some(version) = &self.version {
            table.insert("version", version.as_str().into());
        }
        if let Some(path) = &self.path {
            table.insert("path", path.as_str().into());
        }
        if let Some(git) = &self.git {
            table.insert("git", git.as_str().into());
        }
        if let Some(branch) = &self.branch {
            table.insert("branch", branch.as_str().into());
        }
        if let Some(tag) = &self.tag {
            table.insert("tag", tag.as_str().into());
        }
        if let Some(rev) = &self.rev {
            table.insert("rev", rev.as_str().into());
        }

        Item::Value(toml_edit::Value::InlineTable(table))
    }
}

pub struct TestFixture {
    temp_dir: TempDir,
}

impl TestFixture {
    pub fn new() -> Self {
        Self {
            temp_dir: TempDir::new().expect("create temp dir"),
        }
    }

    fn root(&self) -> &Path {
        self.temp_dir.path()
    }

    pub fn workspace(&self, name: impl Into<String>) -> WorkspaceBuilder<'_> {
        WorkspaceBuilder::new(self, name)
    }

    pub fn project(&self, name: impl Into<String>) -> ProjectBuilder<'_> {
        ProjectBuilder::new(self, name)
    }
}

pub struct WorkspaceBuilder<'a> {
    fixture: &'a TestFixture,
    name: String,
    members: Vec<MemberSpec>,
    workspace_dependencies: BTreeMap<String, DependencySpec>,
}

struct MemberSpec {
    name: String,
    version: String,
    edition: String,
}

impl<'a> WorkspaceBuilder<'a> {
    fn new(fixture: &'a TestFixture, name: impl Into<String>) -> Self {
        Self {
            fixture,
            name: name.into(),
            members: Vec::new(),
            workspace_dependencies: BTreeMap::new(),
        }
    }

    pub fn member(mut self, name: impl Into<String>, version: impl Into<String>) -> Self {
        let name_str = name.into();
        let version_str = version.into();
        self.workspace_dependencies.insert(
            name_str.clone(),
            DependencySpec::version(version_str.clone()),
        );
        self.members.push(MemberSpec {
            name: name_str,
            version: version_str,
            edition: "2021".to_string(),
        });
        self
    }

    pub fn build(self) -> Workspace {
        let workspace_path = self.fixture.root().join(&self.name);
        fs::create_dir(&workspace_path).expect("create workspace root");

        let mut doc = toml_edit::DocumentMut::new();
        {
            let workspace_table = doc
                .entry("workspace")
                .or_insert(Item::Table(Table::new()))
                .as_table_mut()
                .expect("workspace table");

            let mut members = toml_edit::Array::new();
            for member in &self.members {
                members.push(format!("crates/{}", member.name));
            }

            workspace_table.insert("members", Item::Value(toml_edit::Value::Array(members)));

            if !self.workspace_dependencies.is_empty() {
                let deps_table = workspace_table
                    .entry("dependencies")
                    .or_insert(Item::Table(Table::new()))
                    .as_table_mut()
                    .expect("dependencies table");

                for (name, spec) in &self.workspace_dependencies {
                    deps_table.insert(name, spec.to_item());
                }
            }
        }

        fs::write(workspace_path.join("Cargo.toml"), doc.to_string())
            .expect("write workspace manifest");

        let crates_dir = workspace_path.join("crates");
        fs::create_dir(&crates_dir).expect("create crates dir");

        for member in &self.members {
            let crate_dir = crates_dir.join(&member.name);
            fs::create_dir(&crate_dir).expect("create crate dir");

            let manifest = format!(
                "[package]\nname = \"{}\"\nversion = \"{}\"\nedition = \"{}\"\n",
                member.name, member.version, member.edition
            );
            fs::write(crate_dir.join("Cargo.toml"), manifest).expect("write crate manifest");

            let src_dir = crate_dir.join("src");
            fs::create_dir(&src_dir).expect("create src dir");
            fs::write(src_dir.join("lib.rs"), "").expect("write lib");
        }

        Workspace {
            root: workspace_path.clone(),
            manifest_path: workspace_path.join("Cargo.toml"),
        }
    }
}

pub struct Workspace {
    root: PathBuf,
    manifest_path: PathBuf,
}

impl Workspace {
    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn read_manifest(&self) -> String {
        fs::read_to_string(&self.manifest_path).expect("read workspace manifest")
    }
}

pub struct ProjectBuilder<'a> {
    fixture: &'a TestFixture,
    name: String,
    dependencies: BTreeMap<String, DependencySpec>,
}

impl<'a> ProjectBuilder<'a> {
    fn new(fixture: &'a TestFixture, name: impl Into<String>) -> Self {
        Self {
            fixture,
            name: name.into(),
            dependencies: BTreeMap::new(),
        }
    }

    pub fn dep(mut self, name: impl Into<String>, spec: DependencySpec) -> Self {
        self.dependencies.insert(name.into(), spec);
        self
    }

    pub fn dep_version(self, name: impl Into<String>, version: impl Into<String>) -> Self {
        self.dep(name, DependencySpec::version(version.into()))
    }

    pub fn build(self) -> Project {
        let project_path = self.fixture.root().join(&self.name);
        fs::create_dir(&project_path).expect("create project dir");

        let mut doc = toml_edit::DocumentMut::new();
        {
            let package_table = doc
                .entry("package")
                .or_insert(Item::Table(Table::new()))
                .as_table_mut()
                .expect("package table");
            package_table["name"] = toml_edit::value(self.name.clone());
            package_table["version"] = toml_edit::value("0.1.0");
            package_table["edition"] = toml_edit::value("2021");
        }

        if !self.dependencies.is_empty() {
            let dependencies_table = doc
                .entry("dependencies")
                .or_insert(Item::Table(Table::new()))
                .as_table_mut()
                .expect("deps table");
            for (name, spec) in self.dependencies {
                dependencies_table.insert(&name, spec.to_item());
            }
        }

        fs::write(project_path.join("Cargo.toml"), doc.to_string())
            .expect("write project manifest");

        let src_dir = project_path.join("src");
        fs::create_dir(&src_dir).expect("create src dir");
        fs::write(src_dir.join("main.rs"), "fn main() {}\n").expect("write main");

        Project {
            manifest_path: project_path.join("Cargo.toml"),
        }
    }
}

pub struct Project {
    manifest_path: PathBuf,
}

impl Project {
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn read_manifest(&self) -> String {
        fs::read_to_string(&self.manifest_path).expect("read manifest")
    }

    pub fn write_manifest(&self, contents: &str) {
        fs::write(&self.manifest_path, contents).expect("write manifest");
    }

    pub fn append_manifest(&self, contents: &str) {
        let mut existing = self.read_manifest();
        existing.push_str(contents);
        self.write_manifest(&existing);
    }
}
