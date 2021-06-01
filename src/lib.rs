// Copyright (C) 2020-2021 Andy Kurnia.

use rand::prelude::*;
use wasm_bindgen::prelude::*;

#[macro_use]
mod error;

macro_rules! mod_many {
  ($($mod: ident)+) => {
    $(#[allow(dead_code)] mod $mod;)+
  };
}
mod_many!(alphabet bag bites board_layout display fash game_config game_state game_timers kibitzer klv kwg matrix move_filter movegen play_scorer prob simmer stats);

macro_rules! console_log {
    ($($t:tt)*) => (web_sys::console::log_1(&format_args!($($t)*).to_string().into()))
}

macro_rules! return_js_error {
    ($error:expr) => {
        return Err($error.into());
    };
}

#[wasm_bindgen(start)]
pub fn do_this_on_startup() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGame".into(),
        game_config::make_common_english_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "WordSmog".into(),
        game_config::make_jumbled_english_game_config().into(),
    );
}

fn err_to_str<T: std::fmt::Debug>(x: T) -> String {
    format!("{:?}", x)
}

// tile numbering follows alphabet order (not necessarily unicode order).
// rack: array of numbers. 0 for blank, 1 for A.
// board: 2D array of numbers. 0 for empty, 1 for A, -1 for blank-as-A.
// count: maximum number of moves returned.
// (note: equal moves are not stably sorted;
//  different counts may tie-break the last move differently.)
// lexicon: kwg.
// leave: klv.
// rules: hardcoded on startup.
#[derive(serde::Deserialize)]
struct AnalyzeRequest {
    rack: Vec<u8>,
    #[serde(rename = "board")]
    board_tiles: Vec<Vec<i8>>,
    #[serde(rename = "count")]
    max_gen: usize,
    lexicon: String,
    leave: String,
    rules: String,
}

#[derive(serde::Deserialize)]
struct ScoreRequest {
    rack: Vec<u8>,
    #[serde(rename = "board")]
    board_tiles: Vec<Vec<i8>>,
    lexicon: String,
    leave: String,
    rules: String,
    plays: Vec<kibitzer::JsonPlay>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(tag = "result")]
pub enum ScoredPlay {
    #[serde(rename = "error")]
    Error { error: String },
    #[serde(rename = "scored")]
    Scored {
        canonical: bool,
        valid: bool,
        #[serde(flatten)]
        json_play_with_equity: kibitzer::JsonPlayWithEquity,
    },
}

#[derive(serde::Deserialize)]
struct SimPrepareRequest {
    rack: Vec<u8>,
    #[serde(rename = "board")]
    board_tiles: Vec<Vec<i8>>,
    #[serde(rename = "count")]
    max_gen: usize,
    lexicon: String,
    leave: String,
    rules: String,
    #[serde(rename = "plies")]
    num_sim_plies: usize,
    turn: u8,
    rack_sizes: Vec<u8>,
    scores: Vec<i16>,
    zero_turns: u16,
}

struct Candidate {
    play_index: usize,
    stats: stats::Stats,
}

// ongoing Sim Processes, only dropped on request.
struct SimProc {
    initial_board_tiles: Vec<u8>,
    game_config: std::sync::Arc<game_config::GameConfig<'static>>,
    kwg: std::sync::Arc<kwg::Kwg>,
    klv: std::sync::Arc<klv::Klv>,
    simmer: simmer::Simmer,
    plays: Vec<movegen::ValuedMove>,
    candidates: Vec<Candidate>,
    play_finder: std::collections::HashMap<movegen::Play, usize>,
}

type WasmCache<T> = std::sync::RwLock<std::collections::HashMap<String, std::sync::Arc<T>>>;
type WasmCacheInt<T> = std::sync::RwLock<std::collections::HashMap<usize, std::sync::Arc<T>>>;

lazy_static::lazy_static! {
    static ref CACHED_KWG: WasmCache<kwg::Kwg> = Default::default();
    static ref CACHED_KLV: WasmCache<klv::Klv> = Default::default();
    static ref CACHED_GAME_CONFIG: WasmCache<game_config::GameConfig<'static>> = Default::default();
    static ref SIM_PROCS: WasmCacheInt<std::sync::RwLock<SimProc>> = Default::default();
}

macro_rules! get_wasm_cache {
    ($cache: expr, $key: expr, $err: expr) => {
        $cache
            .read()
            .map_err(err_to_str)?
            .get($key)
            .ok_or($err)?
            .clone()
    };
}

macro_rules! use_wasm_cache {
    ($var: ident, $cache: expr, $key: expr) => {
        let $var = get_wasm_cache!($cache, $key, concat!("missing ", stringify!($var)));
    };
}

thread_local! {
    static RNG: std::cell::RefCell<Box<dyn RngCore>> =
        std::cell::RefCell::new(Box::new(rand_chacha::ChaCha20Rng::from_entropy()));
}

#[wasm_bindgen]
pub fn precache_kwg(key: String, value: &[u8]) {
    CACHED_KWG
        .write()
        .unwrap()
        .insert(key, kwg::Kwg::from_bytes_alloc(value).into());
}

#[wasm_bindgen]
pub fn precache_klv(key: String, value: &[u8]) {
    CACHED_KLV
        .write()
        .unwrap()
        .insert(key, klv::Klv::from_bytes_alloc(value).into());
}

#[wasm_bindgen]
pub async fn analyze(req_str: String) -> Result<JsValue, JsValue> {
    let req = serde_json::from_str::<AnalyzeRequest>(&req_str).map_err(err_to_str)?;

    use_wasm_cache!(kwg, CACHED_KWG, &req.lexicon);
    use_wasm_cache!(klv, CACHED_KLV, &req.leave);
    use_wasm_cache!(game_config, CACHED_GAME_CONFIG, &req.rules);

    let mut kibitzer = kibitzer::Kibitzer::new();
    kibitzer
        .prepare(&game_config, &req.rack, &req.board_tiles)
        .map_err(err_to_str)?;

    let mut move_generator = movegen::KurniaMoveGenerator::new(&game_config);
    let board_snapshot = &movegen::BoardSnapshot {
        board_tiles: &kibitzer.board_tiles,
        game_config: &game_config,
        kwg: &kwg,
        klv: &klv,
    };

    move_generator
        .async_gen_moves_filtered(
            board_snapshot,
            &req.rack,
            req.max_gen,
            false,
            |_down: bool, _lane: i8, _idx: i8, _word: &[u8], _score: i16, _rack_tally: &[u8]| true,
            |leave_value: f32| leave_value,
            || wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&JsValue::NULL)),
        )
        .await;
    let plays = &move_generator.plays;

    if false {
        console_log!("found {} moves", plays.len());
        for play in plays.iter() {
            console_log!("{} {}", play.equity, play.play.fmt(board_snapshot));
        }
    }

    let result = plays
        .iter()
        .map(|x| x.into())
        .collect::<Vec<kibitzer::JsonPlayWithEquity>>();

    Ok(serde_json::to_string(&result).map_err(err_to_str)?.into())
}

#[wasm_bindgen]
pub fn play_score(req_str: String) -> Result<JsValue, JsValue> {
    let req = serde_json::from_str::<ScoreRequest>(&req_str).map_err(err_to_str)?;

    use_wasm_cache!(kwg, CACHED_KWG, &req.lexicon);
    use_wasm_cache!(klv, CACHED_KLV, &req.leave);
    use_wasm_cache!(game_config, CACHED_GAME_CONFIG, &req.rules);

    let mut game_state = game_state::GameState::new(&game_config);
    let mut kibitzer = kibitzer::Kibitzer::new();
    kibitzer
        .prepare(&game_config, &req.rack, &req.board_tiles)
        .map_err(err_to_str)?;
    game_state
        .board_tiles
        .copy_from_slice(&kibitzer.board_tiles);
    let mut num_unseen_tiles = kibitzer
        .available_tally
        .iter()
        .map(|&x| x as usize)
        .sum::<usize>();
    let mut unseen_tiles = (0u8..)
        .zip(kibitzer.available_tally.iter())
        .flat_map(|(tile, &count)| std::iter::repeat(tile).take(count as usize));
    for i in 1..game_config.num_players() as usize {
        let len_this_rack = std::cmp::min(num_unseen_tiles, game_config.rack_size() as usize);
        let rack = &mut game_state.players[i].rack;
        rack.clear();
        rack.reserve(len_this_rack);
        for _ in 0..len_this_rack {
            rack.push(unseen_tiles.next().unwrap());
        }
        num_unseen_tiles -= len_this_rack;
    }
    game_state.bag.0.clear();
    game_state.bag.0.reserve(num_unseen_tiles);
    game_state.bag.0.extend(unseen_tiles);
    game_state.players[0].rack.clear();
    game_state.players[0].rack.extend(&req.rack);

    let board_snapshot = &movegen::BoardSnapshot {
        board_tiles: &kibitzer.board_tiles,
        game_config: &game_config,
        kwg: &kwg,
        klv: &klv,
    };

    let mut ps = play_scorer::PlayScorer::new();
    let result = req
        .plays
        .iter()
        .map(|json_play| {
            let play = movegen::Play::from(json_play);
            match ps.validate_play(board_snapshot, &game_state, &play) {
                Err(err) => ScoredPlay::Error {
                    error: err.to_string(),
                },
                Ok(adjusted_play) => {
                    let is_canonical = adjusted_play.is_none();
                    let mut canonical_play = adjusted_play.unwrap_or(play);
                    let recounted_score = ps.compute_score(board_snapshot, &canonical_play);
                    match canonical_play {
                        movegen::Play::Exchange { .. } => {}
                        movegen::Play::Place { ref mut score, .. } => {
                            if *score != recounted_score {
                                *score = recounted_score;
                            }
                        }
                    };
                    let recounted_equity = ps.compute_equity(
                        board_snapshot,
                        &game_state,
                        &canonical_play,
                        1.0,
                        recounted_score,
                    );
                    ScoredPlay::Scored {
                        canonical: is_canonical,
                        valid: ps.words_are_valid(board_snapshot, &canonical_play),
                        json_play_with_equity: kibitzer::JsonPlayWithEquity {
                            equity: recounted_equity,
                            play: (&canonical_play).into(),
                        },
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    Ok(serde_json::to_string(&result).map_err(err_to_str)?.into())
}

//#[wasm_bindgen]
pub fn sim_prepare(req_str: &str) -> Result<JsValue, JsValue> {
    let req = serde_json::from_str::<SimPrepareRequest>(req_str).map_err(err_to_str)?;

    use_wasm_cache!(kwg, CACHED_KWG, &req.lexicon);
    use_wasm_cache!(klv, CACHED_KLV, &req.leave);
    use_wasm_cache!(game_config, CACHED_GAME_CONFIG, &req.rules);

    if req.rack.len() > game_config.rack_size() as usize {
        // It is intentional that analyze() doesn't enforce this.
        return_js_error!(format!(
            "rack: max {} tiles, found {} tiles",
            game_config.rack_size(),
            req.rack.len()
        ));
    }
    if req.turn >= game_config.num_players() {
        return_js_error!(format!(
            "turn: must be >= 0 and < {}, found {}",
            game_config.num_players(),
            req.turn
        ));
    }
    if req.rack_sizes.len() != game_config.num_players() as usize {
        return_js_error!(format!(
            "rack_sizes: length must be {}, found {}",
            game_config.num_players(),
            req.rack_sizes.len(),
        ));
    }
    if !req
        .rack_sizes
        .iter()
        .all(|&x| x <= game_config.rack_size() as u8)
    {
        return_js_error!(format!(
            "rack_sizes: each element must be <= {}, found {:?}",
            game_config.rack_size(),
            req.rack_sizes,
        ));
    }
    if req.rack_sizes[req.turn as usize] as usize != req.rack.len() {
        return_js_error!(format!(
            "rack_sizes[turn={}]={}, but rack length is {}",
            req.turn,
            req.rack_sizes[req.turn as usize],
            req.rack.len(),
        ));
    }
    if req.scores.len() != game_config.num_players() as usize {
        return_js_error!(format!(
            "scores: length must be {}, found {}",
            game_config.num_players(),
            req.scores.len(),
        ));
    }

    let mut kibitzer = kibitzer::Kibitzer::new();
    kibitzer
        .prepare(&game_config, &req.rack, &req.board_tiles)
        .map_err(err_to_str)?;

    let mut game_state = game_state::GameState::new(&game_config);
    game_state.turn = req.turn;
    game_state.zero_turns = req.zero_turns;
    game_state.players[req.turn as usize]
        .rack
        .extend_from_slice(&req.rack);
    game_state
        .board_tiles
        .copy_from_slice(&kibitzer.board_tiles);
    game_state.bag.0.clear();
    game_state
        .bag
        .0
        .reserve(kibitzer.available_tally.iter().map(|&x| x as usize).sum());
    game_state.bag.0.extend(
        (0u8..)
            .zip(kibitzer.available_tally.iter())
            .flat_map(|(tile, &count)| std::iter::repeat(tile).take(count as usize)),
    );
    RNG.with(|rng| {
        game_state.bag.shuffle(&mut *rng.borrow_mut());
    });
    let bag_size = game_state.bag.0.len();
    for i in 0..game_config.num_players() as usize {
        game_state.players[i].score = req.scores[i];
        if i != req.turn as usize {
            game_state
                .bag
                .replenish(&mut game_state.players[i].rack, req.rack_sizes[i] as usize);
        }
        if game_state.players[i].rack.len() != req.rack_sizes[i] as usize {
            return_js_error!(format!(
                "unable to draw rack_sizes {:?} from {} unplayed tiles",
                req.rack_sizes,
                bag_size + req.rack.len(),
            ));
        }
    }

    let mut move_generator = movegen::KurniaMoveGenerator::new(&game_config);
    let board_snapshot = &movegen::BoardSnapshot {
        board_tiles: &kibitzer.board_tiles,
        game_config: &game_config,
        kwg: &kwg,
        klv: &klv,
    };
    move_generator.gen_moves_unfiltered(board_snapshot, &req.rack, req.max_gen, false);

    let mut sim_proc = SimProc {
        initial_board_tiles: kibitzer.board_tiles,
        game_config: game_config.clone(),
        kwg: kwg.clone(),
        klv: klv.clone(),
        plays: std::mem::take(&mut move_generator.plays),
        candidates: (0..move_generator.plays.len())
            .map(|idx| Candidate {
                play_index: idx,
                stats: stats::Stats::new(),
            })
            .collect(),
        simmer: simmer::Simmer::new(&game_config),
        play_finder: Default::default(),
    };
    for (i, play) in sim_proc.plays.iter().enumerate() {
        sim_proc.play_finder.insert(play.play.clone(), i);
    }
    sim_proc
        .simmer
        .prepare(&game_config, &game_state, req.num_sim_plies);

    let mut sim_pid;
    {
        let mut sim_procs = SIM_PROCS.write().map_err(err_to_str)?;
        sim_pid = sim_procs.len();
        loop {
            if let std::collections::hash_map::Entry::Vacant(entry) = sim_procs.entry(sim_pid) {
                entry.insert(std::sync::Arc::new(std::sync::RwLock::new(sim_proc)));
                break;
            }
            sim_pid -= 1;
        }
    }

    Ok((sim_pid as f64).into())
}

//#[wasm_bindgen]
pub fn sim_test(sim_pid: usize) -> Result<JsValue, JsValue> {
    let sim_procs_lock = SIM_PROCS.read().map_err(err_to_str)?;
    let mut sim_proc = sim_procs_lock
        .get(&sim_pid)
        .ok_or("bad sim_pid")?
        .write()
        .map_err(err_to_str)?;

    if false {
        sim_proc.candidates.clear();
        let _ = sim_proc.candidates.first().map(|x| &x.stats);
    }

    if false {
        return Ok(format!(
            "{:?}",
            sim_proc
                .candidates
                .iter()
                .map(|x| x.play_index)
                .collect::<Box<_>>()
        )
        .into());
    }

    let plays = std::mem::take(&mut sim_proc.plays);
    let game_config = sim_proc.game_config.clone();
    let kwg = sim_proc.kwg.clone();
    let klv = sim_proc.klv.clone();
    if false {
        console_log!("begin!");
    }
    let mut stats = stats::Stats::new();
    for _num in 1..=1000 {
        sim_proc.simmer.prepare_iteration();
        for play in plays.iter() {
            let game_ended = sim_proc
                .simmer
                .simulate(&game_config, &kwg, &klv, &play.play);
            let final_spread = sim_proc.simmer.final_equity_spread();
            let win_prob = sim_proc.simmer.compute_win_prob(game_ended, final_spread);
            let sim_spread = final_spread - sim_proc.simmer.initial_score_spread as f32;
            if false {
                let board_snapshot = &movegen::BoardSnapshot {
                    board_tiles: &sim_proc.initial_board_tiles,
                    game_config: &sim_proc.game_config,
                    kwg: &sim_proc.kwg,
                    klv: &sim_proc.klv,
                };
                console_log!(
                    "{} {} gets {}",
                    play.equity,
                    play.play.fmt(board_snapshot),
                    sim_spread as f64 + win_prob * sim_proc.simmer.win_prob_weightage(),
                );
            }
            stats.update(sim_spread as f64 + win_prob * sim_proc.simmer.win_prob_weightage());
        }
    }
    if false {
        console_log!("end");
        console_log!(
            "c={} m={} sd={}",
            stats.count(),
            stats.mean(),
            stats.standard_deviation()
        );
    }
    sim_proc.plays = plays;

    Ok(JsValue::NULL)
}

// true if dropped, false if did not exist
//#[wasm_bindgen]
pub fn sim_drop(sim_pid: usize) -> Result<JsValue, JsValue> {
    Ok(SIM_PROCS
        .write()
        .map_err(err_to_str)?
        .remove(&sim_pid)
        .is_some()
        .into())
}
