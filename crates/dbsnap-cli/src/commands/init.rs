use anyhow::Result;
use dbsnap_engine::{RepoConfig, Repository};

pub fn run(database_url: Option<String>, schema: String) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let config = RepoConfig { schema, database_url };
    let repo = Repository::init(&cwd, config)?;

    println!("Initialized empty dbsnap repository in {}", repo.path().display());
    if repo.config().database_url.is_none() {
        println!("note: no connection string stored; set DATABASE_URL before `dbsnap commit`.");
    }
    Ok(())
}
