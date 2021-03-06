use crate::dates;
use crate::errors::{GitTerminalError, RepositoryError};
use crate::options::Options;
use crate::writer;
use failure::Error;
use git2::ObjectType::Commit;
use git2::ResetType::Mixed;
use git2::{Oid, Repository};
use rand::distributions::WeightedIndex;
use rand::prelude::*;
use std::path::PathBuf;
use chrono::{DateTime, Local, Duration, NaiveTime};

/// A Committer does the work of issuing git commits.
pub struct Committer {
    tree: String,
    author: String,
    message: String,
    days_to_commit: i64,
    start_datetime: DateTime<Local>,
    start_hour: NaiveTime,
    end_hour: NaiveTime,
    repo: Repository,
}

impl Committer {
    /// Creates a new Committer.
    pub fn new(options: Options) -> Result<Committer, Error> {
        let mut repo = Committer::get_repository(&options.repo)?;
        let tree = Committer::create_tree(&mut repo)?;
        let author = Committer::get_author(&repo)?;

        let days_to_commit = (365.0 * options.yrs_ago).round() as i64;
        let now = Local::now();
        let corrected_now = now - (now.time() - options.start);
        let start_datetime = corrected_now - Duration::days(days_to_commit);

        return Ok(Committer {
            tree,
            author,
            message: options.msg,
            days_to_commit,
            start_datetime,
            start_hour: options.start,
            end_hour: options.end,
            repo,
        });
    }

    /// This method can be called to write all commits from yrs ago to current date.
    pub fn commit_all(&self) -> Result<(), Error> {
        // Write init commit.
        let mut commit_time = self.start_datetime;
        let mut blob =
            writer::generate_initial_blob(&self.tree, &self.author, &self.message, commit_time)?;
        let mut parent = self.commit_blob(blob.clone().into_bytes())?;
        let work_duration = self.end_hour - self.start_hour;

        // Main loop to write commits up until present day.
        for _ in 1..self.days_to_commit {
            commit_time = commit_time + Duration::days(1);
            let (p, b) = self.commit_from_time(&parent, &blob, commit_time, work_duration)?;
            parent = p;
            blob = b;
        }

        // Reset head at end.
        self.reset_head_to_hash(parent)?;

        Ok(())
    }

    fn commit_from_time(
        &self,
        parent: &Oid,
        blob: &String,
        start_time: DateTime<Local>,
        work_duration: Duration,
    ) -> Result<(Oid, String), Error> {
        let num_of_commits = Committer::gen_rand_num_commits();
        let mut parent = parent.clone();
        let mut blob = blob.clone();


        for i in 0..num_of_commits {
            let commit_time = start_time
                + Duration::seconds(((work_duration.num_seconds() as f64 / num_of_commits as f64) * (i as f64)) as i64);

            if dates::should_skip_date(commit_time.date()) {
                continue;
            }
            blob = writer::generate_non_initial_blob(
                &self.tree,
                &parent.to_string(),
                &self.author,
                &self.message,
                commit_time,
            )?;
            parent = self.commit_blob(blob.clone().into_bytes())?;
        }

        Ok((parent, blob))
    }

    pub fn gen_rand_num_commits() -> i32 {
        // Generate random number of times to commit today.
        // Weight upper and lower numbers more to create believable spread.
        let choices = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
        let weights = [3, 4, 2, 2, 2, 1, 1, 1, 1, 2, 2, 2, 4, 3];
        let dist = WeightedIndex::new(&weights).unwrap();
        let mut rng = rand::thread_rng();
        choices[dist.sample(&mut rng)]
    }

    fn reset_head_to_hash(&self, hash: Oid) -> Result<(), Error> {
        let obj = self
            .repo
            .find_object(hash, Some(Commit))
            .map_err(|_| GitTerminalError::ResetHeadError {})?;

        self.repo
            .reset(&obj, Mixed, None)
            .map_err(|_| GitTerminalError::ResetHeadError {})?;

        Ok(())
    }

    // Commits a blob returning the object id.
    fn commit_blob(&self, blob: Vec<u8>) -> Result<Oid, Error> {
        let oid = self
            .repo
            .odb()
            .map_err(|_| GitTerminalError::CommitObjectError {})?
            .write(Commit, &blob)
            .map_err(|_| GitTerminalError::CommitObjectError {})?;

        Ok(oid)
    }

    fn get_repository(repo: &PathBuf) -> Result<Repository, Error> {
        let repository = Repository::open(&repo).map_err(|_| RepositoryError::OpenError {})?;

        Ok(repository)
    }

    fn get_author(repo: &Repository) -> Result<String, Error> {
        let signature = repo
            .signature()
            .map_err(|_| RepositoryError::SignatureRetrievalError {})?;

        let name = signature
            .name()
            .ok_or(RepositoryError::NameRetrievalError {})?;

        let email = signature
            .email()
            .ok_or(RepositoryError::EmailRetrievalError {})?;

        Ok(format!("{} <{}>", name, email))
    }

    fn create_tree(repository: &mut Repository) -> Result<String, Error> {
        let mut index = repository
            .index()
            .map_err(|_| RepositoryError::FindIndexError {})?;

        let tree = index
            .write_tree()
            .map_err(|_| RepositoryError::TreeWriteError {})?;

        return Ok(format!("{}", tree));
    }
}
