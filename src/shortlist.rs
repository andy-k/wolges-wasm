// Copyright (C) 2026 Andy Kurnia.

use super::{
    CACHED_GAME_CONFIG, CACHED_KWG, CachedKwg, CachedRules, OUT_OF_MEMORY, PlayScoreRoom, SimPlay,
    Taken, equity, err_to_str, fash, game_state, kibitzer, movegen, wasm_bindgen,
};
use std::sync::{Arc, LazyLock, Mutex};
use wasm_bindgen::JsValue;

#[derive(serde::Deserialize)]
struct NewRequest {
    rack: Vec<u8>,
    board: Vec<Vec<i8>>,
    lexicon: String,
    rules: String,
}

// move ids are indices in this append-only list. Solver handles hold their own
// reference so freeing the shortlist handle cannot invalidate a live solver.
pub(crate) type Shared = Arc<Mutex<Shortlist>>;

pub(crate) struct Shortlist {
    rules_name: String,
    rules: Arc<CachedRules>,
    kwg: Arc<CachedKwg>,
    position: game_state::GameState,
    plays: Vec<movegen::Play>,
}

#[derive(Default)]
struct Shortlists {
    made: u32,
    live: fash::MyHashMap<u32, Shared>,
}

static SHORTLISTS: LazyLock<Mutex<Shortlists>> = LazyLock::new(Default::default);

fn no_shortlist(handle: u32) -> String {
    format!("shortlist {handle}: none such, or freed")
}

fn get(handle: u32) -> Result<Shared, String> {
    SHORTLISTS
        .lock()
        .map_err(err_to_str)?
        .live
        .get(&handle)
        .cloned()
        .ok_or_else(|| no_shortlist(handle))
}

pub(crate) fn attach(
    handle: Option<u32>,
    rules: &str,
    kwg: &Arc<CachedKwg>,
    board: &[u8],
    rack: &[u8],
) -> Result<Option<Shared>, String> {
    let Some(handle) = handle else {
        return Ok(None);
    };
    let shared = get(handle)?;
    let mut sorted = rack.to_vec();
    sorted.sort_unstable();
    {
        let shortlist = shared.lock().map_err(err_to_str)?;
        if rules != shortlist.rules_name
            || !Arc::ptr_eq(kwg, &shortlist.kwg)
            || board != &*shortlist.position.board_tiles
            || sorted != shortlist.position.current_player().rack
        {
            return Err(format!(
                "shortlist {handle}: position, rules, or lexicon differ"
            ));
        }
    }
    Ok(Some(shared))
}

pub(crate) fn attached(shared: &Option<Shared>) -> Result<&Shared, String> {
    shared
        .as_ref()
        .ok_or_else(|| "no shortlist attached".into())
}

impl Shortlist {
    pub(crate) fn play(&self, id: u32) -> Result<&movegen::Play, String> {
        self.plays
            .get(id as usize)
            .ok_or_else(|| format!("shortlist move {id}: none such"))
    }
}

#[wasm_bindgen]
pub fn shortlist_new(req_str: String) -> Result<u32, JsValue> {
    Ok(do_new(&req_str)?)
}

pub(crate) fn do_new(req_str: &str) -> Result<u32, String> {
    let mut req = serde_json::from_str::<NewRequest>(req_str).map_err(err_to_str)?;
    let rules = (CACHED_GAME_CONFIG.read().map_err(err_to_str)?)
        .get(&req.rules)
        .cloned()
        .ok_or("missing game_config")?;
    let kwg = (CACHED_KWG.read().map_err(err_to_str)?)
        .get(&req.lexicon)
        .cloned()
        .ok_or("missing kwg")?;
    if !kwg.fits_alphabet(rules.alphabet().len()) {
        return Err(format!(
            "lexicon: has tiles the {:?} rules do not have",
            req.rules
        ));
    }
    let rack_size = rules.rack_size() as usize;
    if req.rack.len() > rack_size {
        return Err(format!(
            "rack: max {rack_size} tiles, found {} tiles",
            req.rack.len()
        ));
    }
    let mut kibitzer = kibitzer::Kibitzer::new();
    kibitzer
        .prepare(&rules, &req.rack, &req.board)
        .map_err(err_to_str)?;
    let unseen: usize = kibitzer.available_tally.iter().map(|&n| n as usize).sum();
    let opponent_tiles = (rules.num_players() - 1) as usize * rack_size;
    let mut position = game_state::GameState::new(&rules);
    position.board_tiles.copy_from_slice(&kibitzer.board_tiles);
    req.rack.sort_unstable();
    position.players[0].rack = req.rack;
    position.bag.set_from_iter(std::iter::repeat_n(
        0,
        unseen.saturating_sub(opponent_tiles),
    ));

    let mut shortlists = SHORTLISTS.lock().map_err(err_to_str)?;
    let handle = shortlists
        .made
        .checked_add(1)
        .ok_or("too many shortlists made")?;
    shortlists.live.try_reserve(1).map_err(|_| OUT_OF_MEMORY)?;
    shortlists.made = handle;
    shortlists.live.insert(
        handle,
        Arc::new(Mutex::new(Shortlist {
            rules_name: req.rules,
            rules,
            kwg,
            position,
            plays: Vec::new(),
        })),
    );
    Ok(handle)
}

// the entries handed back correspond to the order of the input plays.
#[wasm_bindgen]
pub fn shortlist_add(handle: u32, plays_str: String) -> Result<Vec<u32>, JsValue> {
    Ok(do_add(handle, &plays_str)?)
}

pub(crate) fn do_add(handle: u32, plays_str: &str) -> Result<Vec<u32>, String> {
    let requested = serde_json::from_str::<Vec<SimPlay>>(plays_str).map_err(err_to_str)?;
    let shared = get(handle)?;
    let mut shortlist = shared.lock().map_err(err_to_str)?;
    let snapshot = &movegen::BoardSnapshot {
        board_tiles: &shortlist.position.board_tiles,
        game_config: &shortlist.rules,
        kwg: &shortlist.kwg.kwg,
        anagrams: None,
        rack_lengths: None,
        klv: &shortlist.rules.no_leaves,
    };
    let mut room = Taken::<PlayScoreRoom>::from(&shortlist.rules.play_score_rooms)?;
    let scorer = &mut room.ps;
    let mut added = Vec::new();
    let mut ids = Vec::new();
    ids.try_reserve_exact(requested.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    for (i, requested) in requested.iter().enumerate() {
        let play = (|| {
            if let SimPlay::Play {
                word,
                bonus,
                challenged_off,
                ..
            } = requested
            {
                if *bonus != 0 || *challenged_off {
                    return Err("expected a play without challenge adjustments".into());
                }
                if word.contains(&i8::MIN) {
                    return Err("invalid blank tile".into());
                }
            }
            let play = requested.to_play();
            if let movegen::Play::Exchange { tiles } = &play
                && tiles.len() > shortlist.position.bag.len()
            {
                return Err("exchange exceeds the tiles in the bag".into());
            }
            let mut play = scorer
                .validate_play(snapshot, &shortlist.position, &play)
                .map_err(err_to_str)?
                .unwrap_or(play);
            if !scorer.words_are_valid(snapshot, &play) {
                return Err("play makes an invalid word".into());
            }
            let recounted = scorer.compute_score(snapshot, &play);
            if let movegen::Play::Place { score, .. } = &mut play {
                *score = equity::Equity::new(recounted);
            }
            Ok::<_, String>(play)
        })()
        .map_err(|e| format!("plays[{i}]: {e}"))?;

        let id = shortlist
            .plays
            .iter()
            .chain(added.iter())
            .position(|p| *p == play);
        let id = match id {
            Some(id) => id,
            None => {
                let id = shortlist.plays.len() + added.len();
                added.try_reserve(1).map_err(|_| OUT_OF_MEMORY)?;
                added.push(play);
                id
            }
        };
        ids.push(u32::try_from(id).map_err(|_| "too many shortlist moves")?);
    }
    drop(room);
    shortlist
        .plays
        .try_reserve(added.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    shortlist.plays.extend(added);
    Ok(ids)
}

// read the registered plays, in id order.
#[wasm_bindgen]
pub fn shortlist_plays(handle: u32) -> Result<String, JsValue> {
    Ok(do_plays(handle)?)
}

pub(crate) fn do_plays(handle: u32) -> Result<String, String> {
    struct Plays<'a>(&'a [movegen::Play]);
    impl serde::Serialize for Plays<'_> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.collect_seq(self.0.iter().map(kibitzer::JsonPlayRef::from))
        }
    }
    let shortlist = get(handle)?;
    let shortlist = shortlist.lock().map_err(err_to_str)?;
    super::try_json_kept(&shortlist.rules, &Plays(&shortlist.plays))?
        .ok_or_else(|| OUT_OF_MEMORY.into())
}

#[wasm_bindgen]
pub fn shortlist_free(handle: u32) -> Result<(), JsValue> {
    Ok(do_free(handle)?)
}

pub(crate) fn do_free(handle: u32) -> Result<(), String> {
    SHORTLISTS
        .lock()
        .map_err(err_to_str)?
        .live
        .remove(&handle)
        .map(|_| ())
        .ok_or_else(|| no_shortlist(handle))
}
