use std::io::{self, Read};
use std::ops;
use std::process::Child;
use std::thread;

fn tokenize_pipeline(input: &str) -> Result<Vec<String>, String> {
    let raw = shlex::split(input).ok_or_else(|| format!("could not parse pipeline: {}", input))?;

    let mut tokens = Vec::with_capacity(raw.len());

    // NOTE: renormalizing tokens around pipes (e.g. when given a pipe
    // that is not separated by a space `progress |search -es Category`).
    for token in raw.into_iter() {
        if token == "|" {
            tokens.push(token);
        } else if let Some(rest) = token.strip_prefix("|") {
            tokens.push("|".to_string());
            tokens.push(rest.trim().to_string());
        } else if let Some(rest) = token.strip_suffix("|") {
            tokens.push(rest.trim().to_string());
            tokens.push("|".to_string());
        } else {
            tokens.push(token);
        }
    }

    Ok(tokens)
}

pub fn parse_pipeline(input: &str) -> Result<Vec<Vec<String>>, String> {
    let tokens = tokenize_pipeline(input)?;

    Ok(tokens
        .split(|token| token == "|")
        .map(|args| {
            if args.first().map(|arg| arg.as_str()) == Some("xan") {
                args[1..].to_vec()
            } else {
                args.to_vec()
            }
        })
        .collect())
}

// A struct representing a bunch of child processes that must be watched and
// dropped together.
#[derive(Debug)]
pub struct Children(Vec<Child>);

impl Children {
    pub fn wait(&mut self) -> io::Result<()> {
        for child in self.iter_mut() {
            child.wait()?;
        }

        Ok(())
    }

    pub fn kill(&mut self) -> io::Result<()> {
        for child in self.iter_mut() {
            child.kill()?;
        }

        Ok(())
    }

    pub fn check<F>(&mut self, on_error: F) -> bool
    where
        F: Fn(String),
    {
        let mut must_abort = false;

        for child in self.iter_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        must_abort = true;

                        // Reading some stderr
                        let mut stderr_contents = String::new();
                        let stderr = child.stderr.as_mut().unwrap();

                        stderr
                            .take(1024 * 64)
                            .read_to_string(&mut stderr_contents)
                            .unwrap();

                        on_error(stderr_contents);

                        break;
                    }
                }
                Err(_) => {
                    must_abort = true;
                    break;
                }
                _ => (),
            }
        }

        must_abort
    }
}

impl Drop for Children {
    fn drop(&mut self) {
        if thread::panicking() {
            let _ = self.kill();
        } else {
            let _ = self.wait();
        }
    }
}

impl ops::Deref for Children {
    type Target = [Child];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ops::DerefMut for Children {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<Child>> for Children {
    fn from(children: Vec<Child>) -> Self {
        Self(children)
    }
}
