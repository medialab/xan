use std::fmt::Write;

use minus::{page_all, MinusError, Pager as MinusPager};

pub enum Pager {
    Disabled(String),
    Enabled(MinusPager),
}

impl Pager {
    pub fn new(enabled: bool) -> Result<Self, MinusError> {
        if enabled {
            let pager = MinusPager::new();
            pager.horizontal_scroll(true)?;
            Ok(Self::Enabled(pager))
        } else {
            Ok(Self::Disabled(String::new()))
        }
    }

    pub fn print(self) -> Result<(), MinusError> {
        match self {
            Self::Disabled(string) => {
                print!("{}", string);
                Ok(())
            }
            Self::Enabled(pager) => page_all(pager),
        }
    }

    pub fn set_prompt(&mut self, prompt: &str) -> Result<(), MinusError> {
        match self {
            Self::Enabled(pager) => {
                pager.set_prompt(prompt)?;

                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl Write for Pager {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        match self {
            Self::Disabled(string) => write!(string, "{}", s),
            Self::Enabled(pager) => write!(pager, "{}", s),
        }
    }
}
