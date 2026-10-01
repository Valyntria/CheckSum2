use crate::Error;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

#[derive(Default)]
pub struct Jobs {
    next: AtomicU64,
    jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
}
impl Jobs {
    pub fn create(&self) -> String {
        let id = format!("job-{}", self.next.fetch_add(1, Ordering::Relaxed));
        self.jobs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id.clone(), Arc::new(AtomicBool::new(false)));
        id
    }
    pub fn token(&self, id: &str) -> Result<Arc<AtomicBool>, Error> {
        self.jobs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(id)
            .cloned()
            .ok_or_else(Error::cancelled)
    }
    pub fn cancel(&self, id: &str) {
        if let Some(token) = self
            .jobs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id)
        {
            token.store(true, Ordering::Release);
        }
    }
    pub fn finish(&self, id: &str) {
        self.jobs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelling_one_job_does_not_affect_or_revive_another() {
        let jobs = Jobs::default();
        let a = jobs.create();
        let ta = jobs.token(&a).unwrap();
        let b = jobs.create();
        let tb = jobs.token(&b).unwrap();
        jobs.cancel(&a);
        let _c = jobs.create();
        assert!(ta.load(Ordering::Acquire));
        assert!(!tb.load(Ordering::Acquire));
        assert!(jobs.token(&a).is_err());
    }
}
