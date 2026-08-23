use std::time::Instant;

pub(crate) struct VerificationCheck {
    pub generation: u64,
    pub attempt_id: String,
    pub server: String,
}

pub(crate) struct Observation {
    pub generation: u64,
    pub attempt_id: String,
    pub observed_at: Instant,
}

pub(crate) fn belongs_to_attempt(
    observation: &Observation,
    generation: u64,
    attempt_id: &str,
    round_started_at: Option<Instant>,
) -> bool {
    observation.generation == generation
        && observation.attempt_id == attempt_id
        && round_started_at.is_some_and(|started| observation.observed_at >= started)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_observation_from_an_old_attempt() {
        let started = Instant::now();
        let observation = Observation {
            generation: 2,
            attempt_id: "current".to_owned(),
            observed_at: started,
        };
        assert!(belongs_to_attempt(
            &observation,
            2,
            "current",
            Some(started)
        ));
        assert!(!belongs_to_attempt(&observation, 2, "old", Some(started)));
    }
}
