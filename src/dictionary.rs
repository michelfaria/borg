use onig::Regex;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub enum Error {
    IOError(io::Error),
    JSONError(serde_json::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::IOError(ref e) => e.fmt(f),
            Error::JSONError(ref e) => e.fmt(f),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            Error::IOError(ref e) => Some(e),
            Error::JSONError(ref e) => Some(e),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IOError(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Error {
        Error::JSONError(err)
    }
}

type Indices = HashMap<String, Vec<usize>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Dictionary {
    sentences: Vec<String>,
    indices: Indices,
}

impl PartialEq for Dictionary {
    fn eq(&self, other: &Dictionary) -> bool {
        self.sentences == other.sentences && self.indices == other.indices
    }
}

impl Eq for Dictionary {}

impl Dictionary {
    // load loads a dictionary from the specified path.
    // If there is no file at the specified path, it will create a blank
    // dictionary at that location.
    pub fn load(path: &Path) -> Result<Self, Error> {
        if !path.is_file() {
            let d = Dictionary::new_empty();
            d.write_to_disk(&path)?;
            Ok(d)
        } else {
            let data = fs::read_to_string(path)?;
            let dict: Dictionary = serde_json::from_str(&data)?;
            Ok(dict)
        }
    }

    pub fn write_to_disk(&self, path: &Path) -> Result<(), Error> {
        let json = serde_json::to_string(&self)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn new_empty() -> Dictionary {
        Dictionary {
            sentences: vec![],
            indices: HashMap::new(),
        }
    }

    fn reset_indices(&mut self) {
        self.indices = HashMap::new();
    }

    pub fn needs_to_build_indices(&self) -> bool {
        !self.sentences.is_empty() && self.indices.is_empty()
    }

    pub fn rebuild_indices(&mut self) {
        self.reset_indices();
        sort_sentences(&mut self.sentences);

        let mut indices: Indices = HashMap::new();
        self.sentences
            .iter()
            .enumerate()
            .map(|(i, sentence)| (i, sentence.to_lowercase()))
            .for_each(|(i, sentence)| {
                println!("Indexing: {:?}", sentence);
                let words = split_words(&sentence);
                for word in words {
                    insert_word_into_indices(&mut indices, word, i);
                }
            });
        self.indices = indices
    }

    fn knows_sentence(&self, sentence: &str) -> bool {
        self.sentences.iter().any(|x| x == sentence)
    }

    fn knows_word(&self, word: &str) -> bool {
        self.indices.contains_key(word)
    }

    pub fn learn(&mut self, line: &str) -> bool {
        let mut learned_something = false;
        for sentence in split_sentences(&line.to_lowercase()) {
            if self.knows_sentence(sentence) {
                continue;
            }
            self.sentences.push(sentence.to_owned());
            let sentence_index = self.sentences.len() - 1;

            // Update the indices with the sentence's words
            for word in split_words(&sentence) {
                insert_word_into_indices(&mut self.indices, &word, sentence_index);
            }
            learned_something = true;
        }
        learned_something
    }

    pub fn respond_to(&self, line: &str, rng: &mut dyn RngCore) -> Option<String> {
        let known_words = self.known_words(line);
        if known_words.is_empty() {
            None
        } else {
            let pivot = &known_words[rng.next_u64() as usize % known_words.len()];
            let sentences_with_word = self.sentences_with_word(pivot);
            if sentences_with_word.len() < 2 {
                None
            } else {
                let s1 = *pick_random(&sentences_with_word, rng);
                let s2 = *pick_random(&sentences_with_word, rng);
                let left = get_words_left_of_pivot(s1, pivot)
                    .unwrap_or_else(|| vec![""])
                    .join(" ");
                let right = get_words_right_of_pivot_inclusive(s2, pivot)
                    .unwrap()
                    .join(" ");
                if left == "" {
                    Some(right)
                } else {
                    Some(format!("{} {}", left, right))
                }
            }
        }
    }

    fn known_words(&self, line: &str) -> Vec<String> {
        split_words(&line.to_lowercase())
            .iter()
            .filter(|s| self.knows_word(s))
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    }

    fn sentences_with_word(&self, word: &str) -> Vec<&str> {
        self.indices
            .get(word)
            .map(|ys| ys.iter().map(|y| self.sentences[*y].as_str()).collect())
            .unwrap_or_else(Vec::new)
    }
}

fn split_sentences(s: &str) -> Vec<&str> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"(?<=[.!?]+)\s+").unwrap();
    }
    RE.split(s).filter(|s| !s.is_empty()).collect()
}

fn split_words(s: &str) -> Vec<&str> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"[,.!?:\s]+").unwrap();
    }
    RE.split(s).filter(|s| !s.is_empty()).collect()
}

fn sort_sentences(sentences: &mut Vec<String>) {
    sentences.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()))
}

fn insert_word_into_indices(indices: &mut Indices, word: &str, sentence_index: usize) {
    let entry = indices.entry(word.to_owned()).or_insert_with(Vec::new);
    if !entry.contains(&sentence_index) {
        entry.push(sentence_index);
    }
}

fn pick_random<'a, T>(v: &'a [T], rng: &mut dyn RngCore) -> &'a T {
    &v[rng.next_u64() as usize % v.len()]
}

fn get_words_left_of_pivot<'a>(line: &'a str, pivot: &'a str) -> Option<Vec<&'a str>> {
    let words = split_words(line);
    words
        .iter()
        .position(|word| word == &pivot)
        .map(|pivot_position| words[0..pivot_position].to_vec())
}

fn get_words_right_of_pivot_inclusive<'a>(line: &'a str, pivot: &'a str) -> Option<Vec<&'a str>> {
    let words = split_words(line);
    words
        .iter()
        .position(|word| word == &pivot)
        .map(|pivot_position| words[pivot_position..words.len()].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_sentences() {
        assert_eq!(
            vec![
                "Hi.",
                "This sentence is going to be split.",
                "We.cant.split.things.that.look.like.urls.",
                "That's a single sentence.",
                "Lol!",
                "A single sentence!!!!",
                "Look at this image: https://imgur.com/gallery/PXSNky0"
            ],
            split_sentences(
                "Hi. This sentence is going to be split. \
                We.cant.split.things.that.look.like.urls. That's a single sentence. \
                Lol! A single sentence!!!! Look at this image: https://imgur.com/gallery/PXSNky0"
            ),
        );
    }

    // This tests that the Dictionary::rebuild_indices function is building indices correctly.
    #[test]
    fn test_dictionary_rebuild_indices() {
        let mut d = Dictionary {
            sentences: vec![
                "this is a test.".to_string(),
                "this is is not a trick!".to_string(), // The double "is" is intentional
                "hello world!".to_string(),
            ],
            indices: hashmap![],
        };
        d.rebuild_indices();

        // Ensure that sentences were sorted after rebuilding incides.
        assert_eq!(
            vec![
                "hello world!".to_string(),
                "this is a test.".to_string(),
                "this is is not a trick!".to_string(),
            ],
            d.sentences
        );

        // Ensure that the indices were correctly built
        assert_eq!(
            hashmap![
                "this".to_string() => vec![1, 2],
                "is".to_string() => vec![1, 2],
                "a".to_string() => vec![1, 2],
                "test".to_string() => vec![1],
                "not".to_string() => vec![2],
                "trick".to_string() => vec![2],
                "hello".to_string() => vec![0],
                "world".to_string() => vec![0]
            ],
            d.indices
        );
    }

    #[test]
    fn test_split_words() {
        assert_eq!(
            vec!["Hello", "world", "This", "is", "a", "test", "I", "am", "a", "test"],
            split_words("...Hello world!!!!This is a test? I.am.a.test.")
        );
    }

    #[test]
    fn test_needs_to_build_indices() {
        // Indices should have to be rebuilt when the bot has sentences,
        // but no indices. On other conditions, it assumes that the indices
        // are correct and that they do not need to be rebuilt.

        assert!(Dictionary {
            sentences: vec!["hello world".to_string()],
            indices: hashmap![],
        }
        .needs_to_build_indices());

        assert!(!Dictionary {
            sentences: vec!["hello world".to_string()],
            indices: hashmap![
                "hello".to_string() => vec![0],
                "world".to_string() => vec![0]
            ],
        }
        .needs_to_build_indices());

        assert!(!Dictionary {
            sentences: vec![],
            indices: hashmap![],
        }
        .needs_to_build_indices());
    }

    #[test]
    fn test_knows_sentence() {
        let d = Dictionary {
            sentences: vec![
                "hello world".to_string(),
                "i am a little teapot.".to_string(),
                "my name is foo...".to_string(),
                "short and stout".to_string(),
            ],
            indices: hashmap![
                "hello".to_string() => vec![0],
                "world".to_string() => vec![0],
                "i".to_string() => vec![1],
                "am".to_string() => vec![1],
                "a".to_string() => vec![1],
                "little".to_string() => vec![1],
                "teapot".to_string() => vec![1],
                "my".to_string() => vec![2],
                "name".to_string() => vec![2],
                "is".to_string() => vec![2],
                "foo".to_string() => vec![2],
                "short".to_string() => vec![3],
                "and".to_string() => vec![3],
                "stout".to_string() => vec![3]
            ],
        };
        assert!(d.knows_sentence(&"my name is foo...".to_string()));
        assert!(d.knows_sentence(&"i am a little teapot.".to_string()));
        assert!(d.knows_sentence(&"short and stout".to_string()));
        assert!(!d.knows_sentence(&"i shouldn't know this".to_string()));
        assert!(!d.knows_sentence(&"".to_string()));
        assert!(!d.knows_sentence(&"0".to_string()));
        assert!(!d.knows_sentence(&"a".to_string()));
    }

    #[test]
    fn test_knows_word() {
        let d = Dictionary {
            sentences: vec![
                "and i am a little teapot".to_string(),
                "my name is josh and i am a little teapot".to_string(),
            ],
            indices: hashmap![
                "and".to_string() => vec![0, 1],
                "i".to_string() => vec![0, 1],
                "am".to_string() => vec![0, 1],
                "a".to_string() => vec![0, 1],
                "little".to_string() => vec![0, 1],
                "teapot".to_string() => vec![0, 1],
                "my".to_string() => vec![1],
                "name".to_string() => vec![1],
                "is".to_string() => vec![1],
                "josh".to_string() => vec![1]
            ],
        };

        assert!(d.knows_word("and"));
        assert!(d.knows_word("teapot"));
        assert!(d.knows_word("josh"));
        assert!(!d.knows_word("rat"));
        assert!(!d.knows_word("dog"));
        assert!(!d.knows_word(" "));
        assert!(!d.knows_word(""));
    }

    #[test]
    fn test_insert_word_into_indices() {
        let mut indices = hashmap![
            "joy".to_string() => vec![1, 2]
        ];
        insert_word_into_indices(&mut indices, "john", 10);
        assert_eq!(
            hashmap![
                "joy".to_string() => vec![1, 2],
                "john".to_string() => vec![10]
            ],
            indices
        );
        insert_word_into_indices(&mut indices, "john", 20);
        assert_eq!(
            hashmap![
                "joy".to_string() => vec![1, 2],
                "john".to_string() => vec![10, 20]
            ],
            indices
        );
        insert_word_into_indices(&mut indices, "joy", 1);
        assert_eq!(
            hashmap![
                "joy".to_string() => vec![1, 2],
                "john".to_string() => vec![10, 20]
            ],
            indices
        );
        insert_word_into_indices(&mut indices, "joy", 6);
        assert_eq!(
            hashmap![
                "joy".to_string() => vec![1, 2, 6],
                "john".to_string() => vec![10, 20]
            ],
            indices
        );
    }

    #[test]
    fn test_learn() {
        let mut dict = Dictionary {
            sentences: vec![],
            indices: hashmap![],
        };
        dict.learn("Hey there, everyone!");
        assert_eq!(
            Dictionary {
                sentences: vec!["hey there, everyone!".to_string()],
                indices: hashmap![
                    "hey".to_string() => vec![0],
                    "there".to_string() => vec![0],
                    "everyone".to_string() => vec![0]
                ]
            },
            dict
        );
        dict.learn("How is everyone doing today?!");
        assert_eq!(
            Dictionary {
                sentences: vec![
                    "hey there, everyone!".to_string(),
                    "how is everyone doing today?!".to_string(),
                ],
                indices: hashmap![
                    "hey".to_string() => vec![0],
                    "there".to_string() => vec![0],
                    "everyone".to_string() => vec![0, 1],
                    "how".to_string() => vec![1],
                    "is".to_string() => vec![1],
                    "doing".to_string() => vec![1],
                    "today".to_string() => vec![1]
                ]
            },
            dict
        );
        dict.learn("I've been doing fine today, what about you?");
        assert_eq!(
            Dictionary {
                sentences: vec![
                    "hey there, everyone!".to_string(),
                    "how is everyone doing today?!".to_string(),
                    "i've been doing fine today, what about you?".to_string()
                ],
                indices: hashmap![
                    "hey".to_string() => vec![0],
                    "there".to_string() => vec![0],
                    "everyone".to_string() => vec![0, 1],
                    "how".to_string() => vec![1],
                    "is".to_string() => vec![1],
                    "doing".to_string() => vec![1, 2],
                    "today".to_string() => vec![1, 2],
                    "i've".to_string() => vec![2],
                    "been".to_string() => vec![2],
                    "fine".to_string() => vec![2],
                    "what".to_string() => vec![2],
                    "about".to_string() => vec![2],
                    "you".to_string() => vec![2]
                ]
            },
            dict
        );
    }

    #[test]
    fn test_respond() {
        let dict = Dictionary {
            sentences: vec![
                "hey there everyone".to_string(),
                "everyone is a crab".to_string(),
                "crabs are great".to_string(),
                "there are many crabs".to_string(),
                "crabs".to_string(),
            ],
            indices: hashmap![
                "hey".to_string() => vec![0],
                "there".to_string() => vec![0, 3],
                "everyone".to_string() => vec![0, 1],
                "is".to_string() => vec![1],
                "a".to_string() => vec![1],
                "crab".to_string() => vec![1],
                "crabs".to_string() => vec![2, 3, 4],
                "are".to_string() => vec![2, 3],
                "great".to_string() => vec![3],
                "many".to_string() => vec![3]
            ],
        };
        use rand::rngs::mock::StepRng;
        assert_eq!(
            Some("everyone".to_string()),
            dict.respond_to("Hey there everyone!", &mut StepRng::new(2, 1))
        );
        assert_eq!(
            Some("hey there everyone".to_string()),
            dict.respond_to("Hey there everyone!", &mut StepRng::new(8, 10))
        );
        assert_eq!(
            None,
            dict.respond_to("hey there crab people", &mut StepRng::new(2, 7))
        );
        assert_eq!(
            Some("crabs".to_string()),
            dict.respond_to("hey there crabs people", &mut StepRng::new(2, 7))
        );
    }

    #[test]
    fn test_known_words() {
        let dict = Dictionary {
            sentences: vec!["hello world!".to_string(), "i love pizza.".to_string()],
            indices: hashmap![
                "hello".to_string() => vec![0],
                "world".to_string() => vec![0],
                "i".to_string() => vec![1],
                "love".to_string() => vec![1],
                "pizza".to_string() => vec![1]
            ],
        };

        let empty: Vec<&str> = vec![];

        assert_eq!(vec!["i", "love", "pizza"], dict.known_words("I Love Pizza"));
        assert_eq!(vec!["i", "pizza"], dict.known_words("I Hate Pizza!"));
        assert_eq!(vec!["i", "love"], dict.known_words("I Love You"));
        assert_eq!(empty, dict.known_words("foo likes cake"));
        assert_eq!(empty, dict.known_words("pizzacake"));
    }

    #[test]
    fn test_sentences_with_word() {
        let dict = Dictionary {
            sentences: vec![
                "hello world!".to_string(),
                "i love pizza.".to_string(),
                "pizza is like, cool".to_string(),
            ],
            indices: hashmap![
                "hello".to_string() => vec![0],
                "world".to_string() => vec![0],
                "i".to_string() => vec![1],
                "love".to_string() => vec![1],
                "pizza".to_string() => vec![1, 2],
                "is".to_string() => vec![2],
                "like".to_string() => vec![2],
                "cool".to_string() => vec![2]
            ],
        };

        let empty: Vec<&str> = vec![];

        assert_eq!(
            vec!["i love pizza.", "pizza is like, cool"],
            dict.sentences_with_word("pizza")
        );
        assert_eq!(vec!["i love pizza."], dict.sentences_with_word("love"));
        assert_eq!(empty, dict.sentences_with_word("nonexisting"));
        assert_eq!(empty, dict.sentences_with_word("luve"));
        assert_eq!(empty, dict.sentences_with_word(""));
    }

    #[test]
    fn test_get_words_left_of_pivot() {
        assert_eq!(
            Some(vec!["this", "is", "a"]),
            get_words_left_of_pivot("this is a test yeah this is a test", "test")
        );
        assert_eq!(
            Some(Vec::<&str>::new()),
            get_words_left_of_pivot("this", "this")
        );
        assert_eq!(
            Some(Vec::<&str>::new()),
            get_words_left_of_pivot("this this", "this")
        );
        assert_eq!(None, get_words_left_of_pivot("i am a little teapot", "fox"));
        assert_eq!(
            None,
            get_words_left_of_pivot("abc def ghi jkl", "abc def" /* not a word */)
        );
    }

    #[test]
    fn test_get_words_right_of_pivot_inclusive() {
        assert_eq!(
            Some(vec!["test", "yeah", "this", "is", "a", "test"]),
            get_words_right_of_pivot_inclusive("this is a test yeah this is a test", "test")
        );
        assert_eq!(
            Some(vec!["this"]),
            get_words_right_of_pivot_inclusive("this", "this")
        );
        assert_eq!(
            Some(vec!["this", "this"]),
            get_words_right_of_pivot_inclusive("this this", "this")
        );
        assert_eq!(None, get_words_left_of_pivot("i am a little teapot", "fox"));
        assert_eq!(
            None,
            get_words_right_of_pivot_inclusive("abc def ghi jkl", "abc def" /* not a word */)
        );
    }
}
