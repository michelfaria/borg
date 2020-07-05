use crate::config::Config;
use crate::dictionary::Dictionary;
use rand::rngs::SmallRng;
use rand_core::SeedableRng;

pub struct SeeBorg {
    config: Config,
    dictionary: Dictionary,
    rng: SmallRng,
}

impl SeeBorg {
    pub fn new(config: Config, dictionary: Dictionary) -> SeeBorg {
        SeeBorg {
            config,
            dictionary,
            rng: SmallRng::from_entropy(),
        }
    }

    pub fn respond_to(&mut self, line: &str) -> Option<String> {
        self.dictionary.respond_to(line, &mut self.rng)
    }

    pub fn learn(&mut self, line: &str) {
        self.dictionary.learn(line);
    }

    pub fn get_telegram_token<'a>(&self) -> Option<&str> {
        self.config
            .telegram
            .as_ref()
            .map(|telegram| telegram.token.as_str())
    }
}
