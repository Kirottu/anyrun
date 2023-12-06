use std::collections::VecDeque;
use std::fs;
use std::env;

use crate::scrubber::DesktopEntry;


pub struct History(VecDeque<DesktopEntry>);

impl History {
    pub fn new() -> Self {
        Self(VecDeque::new())
    }

    pub fn load() -> Self {             

        let path = format!(
            "{}/.cache/anyrun-applications-history",
            env::var("HOME").expect("Unable to determine HOME directory")
        );

        if let Ok(content) = fs::read_to_string(&path) {
            let history: VecDeque<DesktopEntry> = ron::from_str(&content)
            .unwrap_or_else(|why| {
                eprintln!("Error parsing history: {}", why);
                VecDeque::new()
            });            
            return Self(history);
        }

        Self::new()
    }

    pub fn write(&self) {

        let path = format!(
            "{}/.cache/anyrun-applications-history",
            env::var("HOME").expect("Unable to determine HOME directory")
        );

        let content = ron::to_string(&self.0).unwrap_or_else(|why| {
            eprintln!("Error serializing history: {}", why);
            String::new()
        });
        if let Err(why) = fs::write(&path, content) {
            eprintln!("Error writing history: {}", why);
        }
    }

    pub fn add_entry(&mut self, entry: DesktopEntry) {        
        self.0.push_front(entry);
    }

    pub fn truncate(&mut self, max_entries: usize) {
        self.0.truncate(max_entries);
    }

    pub fn get_entry_info(&self, entry: &DesktopEntry) -> Option<(usize, usize)> {        
        let index = self.0.iter().position(|x| x == entry)?;
        let count = self.0.iter().filter(|x| *x == entry).count();
        Some((index, count))
    }
    pub fn count(&self) -> usize {
        self.0.len()
    }
}

