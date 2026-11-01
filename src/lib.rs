// Copyright (C) 2020-2026 Andy Kurnia.

use rand::prelude::*;
use wasm_bindgen::prelude::*;

use wolges::*;

macro_rules! console_log {
    ($($t:tt)*) => (web_sys::console::log_1(&format_args!($($t)*).to_string().into()))
}

macro_rules! return_js_error {
    ($error: expr) => {
        return Err($error.into());
    };
}

// every word graph is kept in kbwg's layout, which holds any graph a kwg does
// as well as the larger ones only a kbwg can. So the move generator, with all
// that is generic over the node, is built once, not once a layout. A kwg is
// read from the caller's bytes a piece at a time, each node put in kbwg's
// layout as it comes (kwg::Kwg::from_reader), so a load holds one copy of it in
// the wasm's memory, not two; a leave table likewise (klv::Klv::from_reader).
type Kwg = kwg::Kwg<kwg::Node24>;
type Klv = klv::Klv<kwg::Node24>;

// the commits this wasm was built from: wolges-wasm's, then wolges's, each with
// the target, as their build.rs files stamp them
#[wasm_bindgen]
pub fn version() -> String {
    format!(
        "wolges-wasm {} wolges {}",
        env!("WOLGES_WASM_VERSION"),
        wolges::VERSION
    )
}

#[wasm_bindgen(start)]
pub fn do_this_on_startup() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    CACHED_KLV.write().unwrap().insert(
        "noleave".into(),
        CachedKlv::new(Klv::empty().map_err(err_to_str)?)?.into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGame".into(),
        game_config::make_english_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "WordSmog".into(),
        game_config::make_jumbled_english_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGameSuper".into(),
        game_config::make_super_english_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "WordSmogSuper".into(),
        game_config::make_jumbled_super_english_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGame/catalan".into(),
        game_config::make_catalan_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "WordSmog/catalan".into(),
        game_config::make_jumbled_catalan_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGameSuper/catalan".into(),
        game_config::make_super_catalan_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "WordSmogSuper/catalan".into(),
        game_config::make_jumbled_super_catalan_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGame/dutch".into(),
        game_config::make_dutch_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "WordSmog/dutch".into(),
        game_config::make_jumbled_dutch_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGame/french".into(),
        game_config::make_french_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "WordSmog/french".into(),
        game_config::make_jumbled_french_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGame/german".into(),
        game_config::make_german_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "WordSmog/german".into(),
        game_config::make_jumbled_german_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGameSuper/german".into(),
        game_config::make_super_german_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "WordSmogSuper/german".into(),
        game_config::make_jumbled_super_german_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGame/norwegian".into(),
        game_config::make_norwegian_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "WordSmog/norwegian".into(),
        game_config::make_jumbled_norwegian_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGame/polish".into(),
        game_config::make_polish_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "WordSmog/polish".into(),
        game_config::make_jumbled_polish_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGame/slovene".into(),
        game_config::make_slovene_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "WordSmog/slovene".into(),
        game_config::make_jumbled_slovene_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGame/spanish".into(),
        game_config::make_spanish_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "WordSmog/spanish".into(),
        game_config::make_jumbled_spanish_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGame/swedish".into(),
        game_config::make_swedish_game_config().into(),
    );
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "WordSmog/swedish".into(),
        game_config::make_jumbled_swedish_game_config().into(),
    );
    Ok(())
}

fn err_to_str<T: std::fmt::Debug>(x: T) -> String {
    format!("{x:?}")
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
        invalid_words: Vec<Vec<u8>>,
        #[serde(flatten)]
        json_play_with_equity: kibitzer::JsonPlayWithEquity,
    },
}

// a cached word graph with its largest tile, found once as it loads, so that
// checking it against a request's rules is one comparison instead of a read of
// every node (kwg::Kwg::fits_alphabet) on every request.
struct CachedKwg {
    kwg: Kwg,
    max_tile: Option<u8>,
}

impl CachedKwg {
    fn new(graph: Kwg) -> Self {
        let max_tile = graph.0.iter().map(kwg::Node::tile).max();
        Self {
            kwg: graph,
            max_tile,
        }
    }

    // kwg::Kwg::fits_alphabet's answer, from the tile found at load
    fn fits_alphabet(&self, alphabet_len: u8) -> bool {
        self.max_tile.is_none_or(|tile| tile < alphabet_len)
    }
}

// the most a leave may be worth either way, in thousandths of a point: half of
// what an equity holds, the other half left for a play's score, which no rules
// here can outgrow (the test below checks each). A leave table with a value
// past it is refused as it loads, so no request needs to check its values.
const LEAVE_LIMIT: i32 = i32::MAX / 2;

// a cached leave table with its largest tile, found once as it loads, so that
// checking it against a request's rules is one comparison, as for a word graph.
struct CachedKlv {
    klv: Klv,
    max_tile: Option<u8>,
}

impl CachedKlv {
    fn new(table: Klv) -> Result<Self, JsValue> {
        let (min_leave, max_leave) = table.leave_range();
        if min_leave < -LEAVE_LIMIT || max_leave > LEAVE_LIMIT {
            return_js_error!(format!(
                "leave: has values from {min_leave} to {max_leave} thousandths of a point, past {LEAVE_LIMIT} either way"
            ));
        }
        Ok(Self {
            max_tile: table.max_tile(),
            klv: table,
        })
    }

    // klv::Klv::fits_alphabet's answer, from the tile found at load
    fn fits_alphabet(&self, alphabet_len: u8) -> bool {
        self.max_tile.is_none_or(|tile| tile < alphabet_len)
    }
}

type WasmCache<T> = std::sync::RwLock<fash::MyHashMap<String, std::sync::Arc<T>>>;

static CACHED_KWG: std::sync::LazyLock<WasmCache<CachedKwg>> =
    std::sync::LazyLock::new(Default::default);
static CACHED_KLV: std::sync::LazyLock<WasmCache<CachedKlv>> =
    std::sync::LazyLock::new(Default::default);
static CACHED_GAME_CONFIG: std::sync::LazyLock<WasmCache<game_config::GameConfig>> =
    std::sync::LazyLock::new(Default::default);

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
    static RNG: std::cell::RefCell<Box<dyn rand::Rng>> = std::cell::RefCell::new(Box::new(
        rand::rngs::ChaCha20Rng::try_from_rng(&mut rand::rngs::SysRng).unwrap(),
    ));
}

// a JS byte array read into wolges's readers a piece at a time, so that a file
// is never copied whole into the wasm's memory beside the table it makes
fn js_pieces(bytes: &js_sys::Uint8Array) -> impl FnMut(usize, &mut [u8]) + '_ {
    |at, piece| {
        bytes
            .subarray(at as u32, (at + piece.len()) as u32)
            .copy_to(piece)
    }
}

// the bytes a precache call is given, a Uint8Array; anything else is an error
// thrown before a byte is read
fn js_bytes(value: JsValue) -> Result<js_sys::Uint8Array, JsValue> {
    value
        .dyn_into::<js_sys::Uint8Array>()
        .map_err(|_| JsValue::from_str("not a Uint8Array"))
}

#[wasm_bindgen]
pub fn precache_kwg(
    key: String,
    #[wasm_bindgen(unchecked_param_type = "Uint8Array")] value: JsValue,
) -> Result<(), JsValue> {
    let value = js_bytes(value)?;
    let graph = Kwg::from_reader::<kwg::Node22>(value.length() as usize, js_pieces(&value))
        .map_err(err_to_str)?;
    CACHED_KWG
        .write()
        .map_err(err_to_str)?
        .insert(key, CachedKwg::new(graph).into());
    Ok(())
}

#[wasm_bindgen]
pub fn precache_kbwg(
    key: String,
    #[wasm_bindgen(unchecked_param_type = "Uint8Array")] value: JsValue,
) -> Result<(), JsValue> {
    let value = js_bytes(value)?;
    let graph = Kwg::from_reader::<kwg::Node24>(value.length() as usize, js_pieces(&value))
        .map_err(err_to_str)?;
    CACHED_KWG
        .write()
        .map_err(err_to_str)?
        .insert(key, CachedKwg::new(graph).into());
    Ok(())
}

#[wasm_bindgen]
pub fn precache_klv(
    key: String,
    #[wasm_bindgen(unchecked_param_type = "Uint8Array")] value: JsValue,
) -> Result<(), JsValue> {
    let value = js_bytes(value)?;
    let table = CachedKlv::new(
        Klv::from_reader::<kwg::Node22>(value.length() as usize, js_pieces(&value))
            .map_err(err_to_str)?,
    )?;
    CACHED_KLV
        .write()
        .map_err(err_to_str)?
        .insert(key, table.into());
    Ok(())
}

// a leave table whose graph is already in kbwg's layout (a kblv2), taken as it
// is, as precache_kbwg takes a kbwg
#[wasm_bindgen]
pub fn precache_kblv2(
    key: String,
    #[wasm_bindgen(unchecked_param_type = "Uint8Array")] value: JsValue,
) -> Result<(), JsValue> {
    let value = js_bytes(value)?;
    let table = CachedKlv::new(
        Klv::from_reader::<kwg::Node24>(value.length() as usize, js_pieces(&value))
            .map_err(err_to_str)?,
    )?;
    CACHED_KLV
        .write()
        .map_err(err_to_str)?
        .insert(key, table.into());
    Ok(())
}

// Forget a cached word graph. Callers that load lexicons on demand need a way
// to give the memory back; without one, a long-lived instance keeps every
// lexicon it has ever been asked for. Returns whether the key was there.
//
// A call already in flight is unaffected: get_wasm_cache! clones the Arc before
// doing any work, so this drops the name and the memory goes back to the
// allocator when the last user is done with it. A later call naming an evicted
// key gets the usual "missing kwg", which is the point -- the failure is loud.
#[wasm_bindgen]
pub fn evict_kwg(key: String) -> Result<bool, JsValue> {
    // a live sim made with the name holds its graph till it is freed, so while
    // one is, the eviction is refused
    let holding = sims_holding(|sim| sim.lexicon == key)?;
    if !holding.is_empty() {
        return_js_error!(format!(
            "lexicon {key:?}: sims {holding:?} hold it; free them first"
        ));
    }
    Ok(CACHED_KWG
        .write()
        .map_err(err_to_str)?
        .remove(&key)
        .is_some())
}

// Same, for a leave-value table. See evict_kwg.
#[wasm_bindgen]
pub fn evict_klv(key: String) -> Result<bool, JsValue> {
    let holding = sims_holding(|sim| sim.leave == key)?;
    if !holding.is_empty() {
        return_js_error!(format!(
            "leave {key:?}: sims {holding:?} hold it; free them first"
        ));
    }
    Ok(CACHED_KLV
        .write()
        .map_err(err_to_str)?
        .remove(&key)
        .is_some())
}

#[wasm_bindgen]
pub fn analyze(req_str: String) -> Result<String, JsValue> {
    let req = serde_json::from_str::<AnalyzeRequest>(&req_str).map_err(err_to_str)?;

    use_wasm_cache!(kwg, CACHED_KWG, &req.lexicon);

    do_analyze(req, &kwg)
}

fn do_analyze(req: AnalyzeRequest, kwg: &CachedKwg) -> Result<String, JsValue> {
    use_wasm_cache!(klv, CACHED_KLV, &req.leave);
    use_wasm_cache!(game_config, CACHED_GAME_CONFIG, &req.rules);

    if !kwg.fits_alphabet(game_config.alphabet().len()) {
        return_js_error!(format!(
            "lexicon: has tiles the {:?} rules do not have",
            req.rules
        ));
    }
    if !klv.fits_alphabet(game_config.alphabet().len()) {
        return_js_error!(format!(
            "leave: has tiles the {:?} rules do not have",
            req.rules
        ));
    }

    let mut kibitzer = kibitzer::Kibitzer::new();
    kibitzer
        .prepare(&game_config, &req.rack, &req.board_tiles)
        .map_err(err_to_str)?;

    let mut move_generator = movegen::KurniaMoveGenerator::new(&game_config);
    let board_snapshot = &movegen::BoardSnapshot {
        board_tiles: &kibitzer.board_tiles,
        game_config: &game_config,
        kwg: &kwg.kwg,
        anagrams: None,
        rack_lengths: None,
        klv: &klv.klv,
    };

    {
        let mut seen_moves = fash::MyHashSet::default();
        let mut alpha_buf = Vec::new();
        let mut dedup = |equity: equity::Equity, play: &movegen::Play| match play {
            movegen::Play::Exchange { .. } => true,
            movegen::Play::Place {
                down,
                lane,
                idx,
                word,
                score,
            } => {
                alpha_buf.clear();
                alpha_buf.extend_from_slice(word);
                alpha_buf.sort_unstable();
                seen_moves.insert((
                    equity.raw(),
                    movegen::Play::Place {
                        down: *down,
                        lane: *lane,
                        idx: *idx,
                        word: alpha_buf[..].into(),
                        score: *score,
                    },
                ))
            }
        };
        move_generator.gen_moves_filtered_lean(
            &movegen::GenMovesParams {
                board_snapshot,
                rack: &req.rack,
                max_gen: req.max_gen,
                num_exchanges_by_this_player: 0, // TODO: this should be specified externally
                pass_policy: movegen::PassPolicy::OnlyWhenForced,
                dynamic_leaves: None,
            },
            movegen::PlacePredicate::AcceptAll,
            klv::AdjustLeave::Identity,
            match game_config.game_rules() {
                game_config::GameRules::Classic => movegen::EquityPredicate::AcceptAll,
                game_config::GameRules::Jumbled => movegen::EquityPredicate::Dyn(&mut dedup),
            },
        );
    }
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

    Ok(serde_json::to_string(&result).map_err(err_to_str)?)
}

#[wasm_bindgen]
pub fn play_score(req_str: String) -> Result<String, JsValue> {
    let req = serde_json::from_str::<ScoreRequest>(&req_str).map_err(err_to_str)?;

    use_wasm_cache!(kwg, CACHED_KWG, &req.lexicon);

    do_play_score(req, &kwg)
}

fn do_play_score(req: ScoreRequest, kwg: &CachedKwg) -> Result<String, JsValue> {
    use_wasm_cache!(klv, CACHED_KLV, &req.leave);
    use_wasm_cache!(game_config, CACHED_GAME_CONFIG, &req.rules);

    if !kwg.fits_alphabet(game_config.alphabet().len()) {
        return_js_error!(format!(
            "lexicon: has tiles the {:?} rules do not have",
            req.rules
        ));
    }
    if !klv.fits_alphabet(game_config.alphabet().len()) {
        return_js_error!(format!(
            "leave: has tiles the {:?} rules do not have",
            req.rules
        ));
    }

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
        .flat_map(|(tile, &count)| std::iter::repeat_n(tile, count as usize));
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
    game_state.bag.set_from_iter(unseen_tiles);
    game_state.players[0].rack.clear();
    game_state.players[0].rack.extend(&req.rack);

    let board_snapshot = &movegen::BoardSnapshot {
        board_tiles: &kibitzer.board_tiles,
        game_config: &game_config,
        kwg: &kwg.kwg,
        anagrams: None,
        rack_lengths: None,
        klv: &klv.klv,
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
                        move_filter::LEAVE_SCALE_DENOM,
                        recounted_score,
                    );
                    let mut invalid_words = Vec::new();
                    ps.find_invalid_words(board_snapshot, &canonical_play, |word| {
                        invalid_words.push(word.into())
                    });
                    ScoredPlay::Scored {
                        canonical: is_canonical,
                        valid: invalid_words.is_empty(),
                        invalid_words,
                        json_play_with_equity: kibitzer::JsonPlayWithEquity {
                            equity: recounted_equity as f32 / equity::SCALE as f32,
                            play: (&canonical_play).into(),
                        },
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    Ok(serde_json::to_string(&result).map_err(err_to_str)?)
}

// an analyzer's calls: the letters that fit every empty square beside a tile,
// and sims a caller steps, stops, resumes, and changes between calls. Requests
// and answers are JSON strings, as analyze's are, and every failure is an error
// the call throws, none a trap; each call is a thin export over a part that
// returns its own errors.

// the tables a request names, as the caches hold them
struct Tables {
    kwg: std::sync::Arc<CachedKwg>,
    klv: std::sync::Arc<CachedKlv>,
    game_config: std::sync::Arc<game_config::GameConfig>,
}

// each checked against the rules: a word graph or leave table with tiles the
// rules lack is refused
fn tables(lexicon: &str, leave: &str, rules: &str) -> Result<Tables, String> {
    let kwg =
        (CACHED_KWG.read().map_err(err_to_str)?.get(lexicon).cloned()).ok_or("missing kwg")?;
    let klv = (CACHED_KLV.read().map_err(err_to_str)?.get(leave).cloned()).ok_or("missing klv")?;
    let game_config = (CACHED_GAME_CONFIG
        .read()
        .map_err(err_to_str)?
        .get(rules)
        .cloned())
    .ok_or("missing game_config")?;
    let alphabet_len = game_config.alphabet().len();
    if !kwg.fits_alphabet(alphabet_len) {
        return Err(format!(
            "lexicon: has tiles the {rules:?} rules do not have"
        ));
    }
    if !klv.fits_alphabet(alphabet_len) {
        return Err(format!("leave: has tiles the {rules:?} rules do not have"));
    }
    Ok(Tables {
        kwg,
        klv,
        game_config,
    })
}

// the letters that, put between the tiles before a square (nearest last) and
// the tiles after it, make a word of the lexicon; under jumbled rules, whose
// graph holds each word's tiles in order, any order of them
fn letters_that_fit(
    kwg: &Kwg,
    jumbled: bool,
    alphabet_len: u8,
    before: &[u8],
    after: &[u8],
    word: &mut Vec<u8>,
) -> Vec<u8> {
    (1..alphabet_len)
        .filter(|&letter| {
            word.clear();
            word.extend_from_slice(before);
            word.push(letter);
            word.extend_from_slice(after);
            if jumbled {
                word.sort_unstable();
            }
            let mut p = 0;
            for &tile in word.iter() {
                p = kwg.seek(p, tile);
                if p < 0 {
                    return false;
                }
            }
            kwg::Node::accepts(&kwg[p])
        })
        .collect()
}

// hooks: a board, as analyze's, with its lexicon and rules
#[derive(serde::Deserialize)]
struct HooksRequest {
    #[serde(rename = "board")]
    board_tiles: Vec<Vec<i8>>,
    lexicon: String,
    rules: String,
}

// an empty square: the letters that make the across run of tiles through it a
// word, and those that make the down run one; null where no tile is beside it
// that way, so no word is made that way
#[derive(serde::Serialize)]
struct HookSquare {
    row: i8,
    col: i8,
    across: Option<Vec<u8>>,
    down: Option<Vec<u8>>,
}

// every empty square with a tile beside it, row by row
#[derive(serde::Serialize)]
struct HooksAnswer {
    squares: Vec<HookSquare>,
}

#[wasm_bindgen]
pub fn hooks(req_str: String) -> Result<String, JsValue> {
    Ok(do_hooks(&req_str)?)
}

fn do_hooks(req_str: &str) -> Result<String, String> {
    let req = serde_json::from_str::<HooksRequest>(req_str).map_err(err_to_str)?;
    let kwg = (CACHED_KWG
        .read()
        .map_err(err_to_str)?
        .get(&req.lexicon)
        .cloned())
    .ok_or("missing kwg")?;
    let game_config = (CACHED_GAME_CONFIG
        .read()
        .map_err(err_to_str)?
        .get(&req.rules)
        .cloned())
    .ok_or("missing game_config")?;
    if !kwg.fits_alphabet(game_config.alphabet().len()) {
        return Err(format!(
            "lexicon: has tiles the {:?} rules do not have",
            req.rules
        ));
    }
    let mut kibitzer = kibitzer::Kibitzer::new();
    kibitzer
        .prepare(&game_config, &[], &req.board_tiles)
        .map_err(err_to_str)?;
    let dim = game_config.board_layout().dim();
    let tile_at = |r: i8, c: i8| {
        if (0..dim.rows).contains(&r) && (0..dim.cols).contains(&c) {
            kibitzer.board_tiles[r as usize * dim.cols as usize + c as usize]
        } else {
            0
        }
    };
    let jumbled = matches!(game_config.game_rules(), game_config::GameRules::Jumbled);
    let alphabet_len = game_config.alphabet().len();
    // the letters of the tiles from (r, c) on, a step of (dr, dc) at a time, as
    // long as there are tiles; a blank's is the letter it stands for
    let run = |mut r: i8, mut c: i8, dr: i8, dc: i8| {
        let mut letters = Vec::new();
        loop {
            r += dr;
            c += dc;
            match tile_at(r, c) {
                0 => return letters,
                tile => letters.push(tile & 0x7f),
            }
        }
    };
    let mut word = Vec::new();
    let mut squares = Vec::new();
    for r in 0..dim.rows {
        for c in 0..dim.cols {
            let beside_a_tile = [(r - 1, c), (r, c - 1), (r, c + 1), (r + 1, c)]
                .iter()
                .any(|&(r, c)| tile_at(r, c) != 0);
            if tile_at(r, c) != 0 || !beside_a_tile {
                continue;
            }
            let mut fit = |dr: i8, dc: i8| {
                let mut before = run(r, c, -dr, -dc);
                before.reverse();
                let after = run(r, c, dr, dc);
                (!before.is_empty() || !after.is_empty()).then(|| {
                    letters_that_fit(&kwg.kwg, jumbled, alphabet_len, &before, &after, &mut word)
                })
            };
            let across = fit(0, 1);
            let down = fit(1, 0);
            squares.push(HookSquare {
                row: r,
                col: c,
                across,
                down,
            });
        }
    }
    serde_json::to_string(&HooksAnswer { squares }).map_err(err_to_str)
}

// the most plies a sim's rollouts may play
const MAX_SIM_PLIES: usize = 16;

// sim_new: a position (rack, board, lexicon, leave, rules, each seat's score in
// points, the seat to move, and the scoreless turns and the passes in a row so
// far; each seat's rack size, by default the mover's rack with full racks for
// the others dealt in turn from the tiles unseen while enough are left), the
// tiles known to be on the opponent's rack (none by default), the plies each
// rollout plays (by default as many as wolges's own sims play), the seed its
// draws come from (a random one if none), and, for a sim run in parts, which
// part of how many this is. A new sim holds no candidates.
#[derive(serde::Deserialize)]
struct SimNewRequest {
    rack: Vec<u8>,
    #[serde(rename = "board")]
    board_tiles: Vec<Vec<i8>>,
    lexicon: String,
    leave: String,
    rules: String,
    scores: Vec<i32>,
    turn: u8,
    zero_turns: u16,
    pass_turns: u16,
    rack_sizes: Option<Vec<u8>>,
    #[serde(default)]
    known: Vec<u8>,
    #[serde(default = "default_plies")]
    plies: usize,
    seed: Option<u64>,
    #[serde(default)]
    part: u64,
    #[serde(default = "one_part")]
    parts: u64,
}

fn default_plies() -> usize {
    move_picker::DEFAULT_NUM_SIM_PLIES
}

fn one_part() -> u64 {
    1
}

// a play as a caller names one to a sim: as play_score takes one, but its score
// may be left out, since the sim counts it again. Other fields are not read, so
// a play as an answer gives it back may be passed as it is.
#[derive(serde::Deserialize)]
#[serde(tag = "action")]
enum SimPlay {
    #[serde(rename = "exchange")]
    Exchange { tiles: Box<[u8]> },
    #[serde(rename = "play")]
    Play {
        down: bool,
        lane: i8,
        idx: i8,
        word: Box<[i8]>,
    },
}

impl SimPlay {
    fn to_play(&self) -> movegen::Play {
        (&match self {
            Self::Exchange { tiles } => kibitzer::JsonPlay::Exchange {
                tiles: tiles.clone(),
            },
            Self::Play {
                down,
                lane,
                idx,
                word,
            } => kibitzer::JsonPlay::Play {
                down: *down,
                lane: *lane,
                idx: *idx,
                word: word.clone(),
                score: 0,
            },
        })
            .into()
    }
}

// sim_step: at most this many iterations, at most for this many milliseconds,
// or both, whichever ends first; iterations 0 reads the sim without stepping
// it. Pruning (by default only for a sim not run in parts) and a play each
// candidate is compared with, on the iterations both ran.
#[derive(serde::Deserialize)]
struct SimStepRequest {
    iterations: Option<u64>,
    ms: Option<f64>,
    prune: Option<bool>,
    against: Option<SimPlay>,
}

// where a sim's candidate stands: rolled out every iteration, held out by the
// caller, or held out by the sim as behind the leader
#[derive(Clone, Copy, PartialEq)]
enum SimState {
    Running,
    Paused,
    Pruned,
}

impl SimState {
    fn name(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Pruned => "pruned",
        }
    }
}

// a ply of a candidate's rollouts, over the rollouts that reached it: the score
// made on it, and whether that play took a whole rack
#[derive(Default)]
struct SimPly {
    score: stats::Stats,
    bingo: stats::Stats,
}

// a sim's candidate: its play as the scorer checked it, its score recounted,
// its static equity, where it stands, its rollout's chance to win and equity on
// each of the sim's iterations from the one it joined at (NaN where it sat one
// out), and the statistics of its rollouts: their objective, their change in
// spread with the leaves left counted (the sim's equity, in points), the chance
// to win, and each ply's
struct SimCandidate {
    play: movegen::Play,
    static_equity: f32,
    state: SimState,
    first_iter: u64,
    wins: Vec<f64>,
    equities: Vec<f64>,
    objective: stats::Stats,
    equity: stats::Stats,
    win: stats::Stats,
    plies: Vec<SimPly>,
}

// a sim a caller steps, stops, resumes, and changes: the position, the tables
// it was made with (held here, so that evicting their names cannot pull them
// away), the simmer prepared on it, the plies a rollout plays, its seed, which
// part of how many it is, its iterations so far, and its candidates
struct Sim {
    lexicon: String,
    leave: String,
    game_config: std::sync::Arc<game_config::GameConfig>,
    kwg: std::sync::Arc<CachedKwg>,
    klv: std::sync::Arc<CachedKlv>,
    game_state: game_state::GameState,
    simmer: simmer::Simmer,
    pristine: game_state::GameState,
    plies: usize,
    seed: u64,
    part: u64,
    parts: u64,
    iterations: u64,
    candidates: Vec<SimCandidate>,
}

// the sims made, by handle; a handle is never used twice
#[derive(Default)]
struct Sims {
    made: u32,
    live: fash::MyHashMap<u32, Sim>,
}

static SIMS: std::sync::LazyLock<std::sync::Mutex<Sims>> =
    std::sync::LazyLock::new(Default::default);

fn no_sim(handle: u32) -> String {
    format!("sim {handle}: none such, or freed")
}

// an iteration's seed from the sim's seed and the iteration's number alone, as
// wolges's move_picker seeds its iterations: a sim's results are the same
// however its steps split it, and sims of other seeds draw apart. A sim run in
// parts numbers its iterations part + 1, part + 1 + parts, and so on, so that
// the parts together run one sim's. The mix is splitmix64's:
// https://prng.di.unimi.it/splitmix64.c
fn iteration_seed(seed: u64, iteration: u64) -> u64 {
    let mut z = seed.wrapping_add(iteration.wrapping_mul(0x9e3779b97f4a7c15));
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

// one rollout of a play, as wolges's move_picker rolls one out, by the move
// generator's lean arm, the one arm the wasm carries: its objective (the change
// in spread in points, plus the chance to win weighted as the simmer weighs
// it), the change in spread, and the chance to win; its plies are the simmer's
// to read until the next
fn rollout(
    simmer: &mut simmer::Simmer,
    game_config: &game_config::GameConfig,
    kwg: &Kwg,
    klv: &Klv,
    play: &movegen::Play,
) -> Result<(f64, i32, f64), String> {
    let game_ended = simmer
        .simulate_lean(game_config, kwg, &anagrams::ArmTables::None, klv, play)
        .map_err(err_to_str)?;
    let final_spread = simmer.final_equity_spread().map_err(err_to_str)?;
    let win = simmer.compute_win_prob(game_ended, final_spread, None);
    let spread = final_spread
        .checked_sub(simmer.initial_score_spread)
        .ok_or("score overflow")?;
    Ok((
        simmer::sim_objective(spread, win, simmer.win_prob_weightage()),
        spread,
        win,
    ))
}

impl Sim {
    // a play as the sim holds it, checked and scored as play_score checks and
    // scores it, with its static equity: refused if it is no play on this
    // position, or makes a word the lexicon lacks
    fn held(&self, sim_play: &SimPlay) -> Result<(movegen::Play, f32), String> {
        let board_snapshot = &movegen::BoardSnapshot {
            board_tiles: &self.game_state.board_tiles,
            game_config: &self.game_config,
            kwg: &self.kwg.kwg,
            anagrams: None,
            rack_lengths: None,
            klv: &self.klv.klv,
        };
        let mut ps = play_scorer::PlayScorer::new();
        let play = sim_play.to_play();
        let mut play = ps
            .validate_play(board_snapshot, &self.game_state, &play)
            .map_err(|e| e.to_string())?
            .unwrap_or(play);
        let score = ps.compute_score(board_snapshot, &play);
        if let movegen::Play::Place { score: s, .. } = &mut play {
            *s = score;
        }
        let mut invalid_words = Vec::<Vec<u8>>::new();
        ps.find_invalid_words(board_snapshot, &play, |word| {
            invalid_words.push(word.into())
        });
        if !invalid_words.is_empty() {
            return Err(format!("makes words the lexicon lacks: {invalid_words:?}"));
        }
        let equity = ps.compute_equity(
            board_snapshot,
            &self.game_state,
            &play,
            move_filter::LEAVE_SCALE_DENOM,
            score,
        );
        Ok((play, equity as f32 / equity::SCALE as f32))
    }

    // up to max iterations, until out_of_time says so before one, while a
    // candidate runs; each rolls out every running candidate on the same draws,
    // and with prune, every PRUNE_CADENCE iterations, those behind the leader
    // are pruned
    fn step(
        &mut self,
        max: u64,
        prune: bool,
        mut out_of_time: impl FnMut() -> bool,
    ) -> Result<u64, String> {
        let z = move_picker::paired_z(move_picker::DEFAULT_NUM_SIM_ITERS);
        let rack_size = self.game_config.rack_size();
        let mut rows = Vec::new();
        let mut plies = Vec::new();
        let mut stepped = 0;
        while stepped < max
            && self.candidates.iter().any(|c| c.state == SimState::Running)
            && !out_of_time()
        {
            let iteration = self.iterations + 1;
            self.simmer.restore_prepared(&self.pristine);
            self.simmer.reseed(iteration_seed(
                self.seed,
                (self.part + 1).wrapping_add(self.parts.wrapping_mul(iteration - 1)),
            ));
            self.simmer.prepare_iteration();
            rows.clear();
            plies.clear();
            for candidate in &self.candidates {
                if candidate.state != SimState::Running {
                    continue;
                }
                let (objective, spread, win) = rollout(
                    &mut self.simmer,
                    &self.game_config,
                    &self.kwg.kwg,
                    &self.klv.klv,
                    &candidate.play,
                )?;
                let played = self.simmer.plies_played();
                let num_plies = played.len().min(self.plies);
                plies.extend_from_slice(&played[..num_plies]);
                rows.push((objective, spread, win, num_plies));
            }
            let mut row = rows.iter();
            let mut at = 0;
            for candidate in &mut self.candidates {
                if candidate.state != SimState::Running {
                    continue;
                }
                let Some(&(objective, spread, win, num_plies)) = row.next() else {
                    break;
                };
                let equity = simmer::spread_points(spread);
                let len = (iteration - candidate.first_iter) as usize;
                candidate.wins.resize(len, f64::NAN);
                candidate.wins.push(win);
                candidate.equities.resize(len, f64::NAN);
                candidate.equities.push(equity);
                candidate.objective.update(objective);
                candidate.equity.update(equity);
                candidate.win.update(win);
                for (ply, played) in candidate.plies.iter_mut().zip(&plies[at..at + num_plies]) {
                    ply.score.update(simmer::spread_points(played.score));
                    ply.bingo
                        .update(if played.tiles == rack_size { 1.0 } else { 0.0 });
                }
                at += num_plies;
            }
            self.iterations = iteration;
            stepped += 1;
            if prune && iteration.is_multiple_of(move_picker::PRUNE_CADENCE) {
                prune_behind(&mut self.candidates, z);
            }
        }
        Ok(stepped)
    }

    fn report(&self, stepped: u64, against: Option<&movegen::Play>) -> SimReport {
        let weight = self.simmer.win_prob_weightage();
        let other = against.and_then(|play| self.candidates.iter().find(|c| c.play == *play));
        let other_objectives = other.map(|other| objectives(other, weight));
        SimReport {
            iterations: self.iterations,
            stepped,
            candidates: (self.candidates.iter())
                .map(|c| SimCandidateReport {
                    play: kibitzer::JsonPlayWithEquity {
                        equity: c.static_equity,
                        play: (&c.play).into(),
                    },
                    state: c.state.name(),
                    n: c.objective.count() as u64,
                    win: (&c.win).into(),
                    equity: (&c.equity).into(),
                    objective: (&c.objective).into(),
                    plies: (c.plies.iter())
                        .map(|ply| SimPlyReport {
                            n: ply.score.count() as u64,
                            score: (&ply.score).into(),
                            bingo: ply.bingo.mean(),
                        })
                        .collect(),
                    against: other.zip(other_objectives.as_deref()).map(
                        |(other, other_objectives)| {
                            let less = |mine: &[f64], others: &[f64]| -> SimCountedStat {
                                (&move_picker::paired_difference(
                                    mine,
                                    c.first_iter,
                                    others,
                                    other.first_iter,
                                ))
                                    .into()
                            };
                            SimAgainst {
                                win: less(&c.wins, &other.wins),
                                equity: less(&c.equities, &other.equities),
                                objective: less(&objectives(c, weight), other_objectives),
                            }
                        },
                    ),
                })
                .collect(),
        }
    }
}

// a chance to win this near 0 or 1 has its game decided: within one game, in a
// whole default sim's iterations, of certain
const DECIDED: f64 = 1.0 / move_picker::DEFAULT_NUM_SIM_ITERS as f64;

// how a chance to win has its game: won, lost, or still open
#[derive(Clone, Copy, PartialEq, Debug)]
enum Outcome {
    Won,
    Lost,
    Open,
}

fn outcome(win: f64) -> Outcome {
    if win >= 1.0 - DECIDED {
        Outcome::Won
    } else if win <= DECIDED {
        Outcome::Lost
    } else {
        Outcome::Open
    }
}

// the running candidate a page sorting by the chance to win lists first: if any
// game is won, the won one of the highest equity; if every game is lost, the
// one of the highest equity; else the one of the highest chance to win
fn leader(candidates: &[SimCandidate]) -> Option<&SimCandidate> {
    let running = || (candidates.iter()).filter(|c| c.state == SimState::Running);
    let by_equity =
        |a: &&SimCandidate, b: &&SimCandidate| a.equity.mean().total_cmp(&b.equity.mean());
    let won = |c: &&SimCandidate| outcome(c.win.mean()) == Outcome::Won;
    if running().any(|c| won(&c)) {
        running().filter(won).max_by(by_equity)
    } else if running().all(|c| outcome(c.win.mean()) == Outcome::Lost) {
        running().max_by(by_equity)
    } else {
        running().max_by(|a, b| a.win.mean().total_cmp(&b.win.mean()))
    }
}

// whether a candidate is compared with the leader by equity: when both games
// are won, or both lost, the chance to win cannot tell them apart
fn by_equity(win: f64, lead_win: f64) -> bool {
    let mine = outcome(win);
    mine != Outcome::Open && mine == outcome(lead_win)
}

// the running candidates behind the leader on the iterations both ran, pruned:
// behind by the chance to win, or, where the chance to win cannot tell them
// apart, by equity, each as wolges's move picker tells a candidate behind
fn prune_behind(candidates: &mut [SimCandidate], z: f64) {
    let Some(lead) = leader(candidates) else {
        return;
    };
    let behind = (candidates.iter())
        .map(|c| {
            let (mine, leads) = if by_equity(c.win.mean(), lead.win.mean()) {
                (&c.equities, &lead.equities)
            } else {
                (&c.wins, &lead.wins)
            };
            c.state == SimState::Running
                && move_picker::behind_paired(mine, c.first_iter, leads, lead.first_iter, z)
        })
        .collect::<Vec<_>>();
    for (candidate, behind) in candidates.iter_mut().zip(behind) {
        if behind {
            candidate.state = SimState::Pruned;
        }
    }
}

// a candidate's objective on each iteration from its first, as its rollout made
// it: its equity plus its chance to win at the sim's weight (NaN where it sat
// one out)
fn objectives(candidate: &SimCandidate, weight: f64) -> Vec<f64> {
    (candidate.equities.iter())
        .zip(&candidate.wins)
        .map(|(&equity, &win)| equity + win * weight)
        .collect()
}

// a statistic's mean and sample variance (0 under two samples), with the
// candidate's n enough to merge sims' results: pooled, n is the sum, the mean
// the n-weighted mean, and the variance as Chan's parallel algorithm has it
#[derive(serde::Serialize)]
struct SimStat {
    mean: f64,
    variance: f64,
}

impl From<&stats::Stats> for SimStat {
    fn from(stats: &stats::Stats) -> Self {
        Self {
            mean: stats.mean(),
            variance: stats.variance(),
        }
    }
}

// a statistic over fewer samples than the candidate's rollouts, with its own n
#[derive(serde::Serialize)]
struct SimCountedStat {
    n: u64,
    mean: f64,
    variance: f64,
}

impl From<&stats::Stats> for SimCountedStat {
    fn from(stats: &stats::Stats) -> Self {
        Self {
            n: stats.count() as u64,
            mean: stats.mean(),
            variance: stats.variance(),
        }
    }
}

// a ply of a candidate's rollouts: how many reached it, the score made on it
// (points), and the share of them whose play took a whole rack
#[derive(serde::Serialize)]
struct SimPlyReport {
    n: u64,
    score: SimStat,
    bingo: f64,
}

// a candidate less the play compared against, on the iterations both ran: in
// the chance to win, in equity, and in objective
#[derive(serde::Serialize)]
struct SimAgainst {
    win: SimCountedStat,
    equity: SimCountedStat,
    objective: SimCountedStat,
}

// a candidate: its play as analyze lists one (its score recounted, its equity
// the static one), where it stands, and its rollouts: their number, chance to
// win, equity (the change in spread in points, the leaves left counted),
// objective, and plies (the first the opponent's reply); with a play to compare
// against, itself less that play
#[derive(serde::Serialize)]
struct SimCandidateReport {
    play: kibitzer::JsonPlayWithEquity,
    state: &'static str,
    n: u64,
    win: SimStat,
    equity: SimStat,
    objective: SimStat,
    plies: Vec<SimPlyReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    against: Option<SimAgainst>,
}

// sim_step's answer: the sim's iterations in all and in this step, and its
// candidates in the order they joined
#[derive(serde::Serialize)]
struct SimReport {
    iterations: u64,
    stepped: u64,
    candidates: Vec<SimCandidateReport>,
}

// a new sim's handle
#[wasm_bindgen]
pub fn sim_new(req_str: String) -> Result<u32, JsValue> {
    Ok(do_sim_new(&req_str)?)
}

fn do_sim_new(req_str: &str) -> Result<u32, String> {
    let req = serde_json::from_str::<SimNewRequest>(req_str).map_err(err_to_str)?;
    let Tables {
        kwg,
        klv,
        game_config,
    } = tables(&req.lexicon, &req.leave, &req.rules)?;
    let num_players = game_config.num_players() as usize;
    let rack_size = game_config.rack_size() as usize;
    if !(1..=MAX_SIM_PLIES).contains(&req.plies) {
        return Err(format!("plies: 1 to {MAX_SIM_PLIES}, found {}", req.plies));
    }
    if req.part >= req.parts {
        return Err(format!(
            "part: must be >= 0 and < parts={}, found {}",
            req.parts, req.part
        ));
    }
    if req.rack.len() > rack_size {
        return Err(format!(
            "rack: max {rack_size} tiles, found {} tiles",
            req.rack.len()
        ));
    }
    let turn = req.turn as usize;
    if turn >= num_players {
        return Err(format!(
            "turn: must be >= 0 and < {num_players}, found {turn}"
        ));
    }
    if req.scores.len() != num_players {
        return Err(format!(
            "scores: length must be {num_players}, found {}",
            req.scores.len()
        ));
    }
    // a request's scores are points; the game keeps millipoints
    let scores = (req.scores.iter())
        .map(|&points| points.checked_mul(equity::SCALE))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            format!(
                "scores: each must be within {} points, found {:?}",
                i32::MAX / equity::SCALE,
                req.scores
            )
        })?;

    let mut kibitzer = kibitzer::Kibitzer::new();
    kibitzer
        .prepare(&game_config, &req.rack, &req.board_tiles)
        .map_err(err_to_str)?;
    // by default the seat to move holds its rack, and the others are dealt in
    // turn from the tiles unseen, a full rack each while enough are left
    let rack_sizes = req.rack_sizes.unwrap_or_else(|| {
        let mut left = kibitzer
            .available_tally
            .iter()
            .map(|&count| count as usize)
            .sum::<usize>();
        (0..num_players)
            .map(|i| {
                if i == turn {
                    req.rack.len() as u8
                } else {
                    let dealt = rack_size.min(left);
                    left -= dealt;
                    dealt as u8
                }
            })
            .collect()
    });
    if rack_sizes.len() != num_players {
        return Err(format!(
            "rack_sizes: length must be {num_players}, found {}",
            rack_sizes.len()
        ));
    }
    if rack_sizes.iter().any(|&n| n as usize > rack_size) {
        return Err(format!(
            "rack_sizes: each must be <= {rack_size}, found {rack_sizes:?}"
        ));
    }
    if rack_sizes[turn] as usize != req.rack.len() {
        return Err(format!(
            "rack_sizes[turn={turn}]={}, but rack length is {}",
            rack_sizes[turn],
            req.rack.len()
        ));
    }
    // the tiles known to be on the opponent's rack: each iteration puts them
    // there first and deals the rest of it from the tiles unseen
    if !req.known.is_empty() {
        if num_players != 2 {
            return Err(format!("known: only with two seats, found {num_players}"));
        }
        let opponent = 1 - turn;
        if req.known.len() > rack_sizes[opponent] as usize {
            return Err(format!(
                "known: {} tiles, but the opponent's rack holds {}",
                req.known.len(),
                rack_sizes[opponent]
            ));
        }
        let mut unseen = kibitzer.available_tally.clone();
        for &tile in &req.known {
            match unseen.get_mut(tile as usize) {
                Some(count) if *count > 0 => *count -= 1,
                _ => {
                    return Err(format!(
                        "known: {:?} are not all among the tiles unseen",
                        req.known
                    ));
                }
            }
        }
    }

    // the bag in tile order and the other seats dealt from it as it lies, so
    // that a seed is all that makes a sim's draws
    let mut game_state = game_state::GameState::new(&game_config);
    game_state.turn = req.turn;
    game_state.zero_turns = req.zero_turns;
    game_state.pass_turns = req.pass_turns;
    game_state
        .board_tiles
        .copy_from_slice(&kibitzer.board_tiles);
    game_state.players[turn].rack.extend_from_slice(&req.rack);
    game_state.bag.set_from_iter(
        (0u8..)
            .zip(kibitzer.available_tally.iter())
            .flat_map(|(tile, &count)| std::iter::repeat_n(tile, count as usize)),
    );
    let unseen = game_state.bag.len();
    for i in 0..num_players {
        game_state.players[i].score = scores[i];
        if i != turn {
            game_state
                .bag
                .replenish(&mut game_state.players[i].rack, rack_sizes[i] as usize, i);
        }
        if game_state.players[i].rack.len() != rack_sizes[i] as usize {
            return Err(format!(
                "rack_sizes: cannot deal {rack_sizes:?} from the {unseen} tiles unseen"
            ));
        }
    }

    let seed = req
        .seed
        .unwrap_or_else(|| RNG.with(|rng| rng.borrow_mut().next_u64()));
    let mut simmer = simmer::Simmer::new(&game_config).map_err(err_to_str)?;
    let known = if req.known.is_empty() {
        Vec::new()
    } else {
        vec![(req.known, 1.0)]
    };
    simmer.prepare(&game_config, &game_state, req.plies, true, &known);
    let pristine = simmer.prepared_state().clone();
    let sim = Sim {
        lexicon: req.lexicon,
        leave: req.leave,
        game_config,
        kwg,
        klv,
        game_state,
        simmer,
        pristine,
        plies: req.plies,
        seed,
        part: req.part,
        parts: req.parts,
        iterations: 0,
        candidates: Vec::new(),
    };

    let mut sims = SIMS.lock().map_err(err_to_str)?;
    let handle = sims.made.checked_add(1).ok_or("too many sims made")?;
    sims.made = handle;
    sims.live.insert(handle, sim);
    Ok(handle)
}

#[wasm_bindgen]
extern "C" {
    // performance.now, whose throw (where there is no performance object) comes
    // back as an error, never unwinding the wasm
    #[wasm_bindgen(catch, js_namespace = performance, js_name = now)]
    fn performance_now() -> Result<f64, JsValue>;
}

// the clock a step's time is read on: performance.now, which only moves
// forward, and Date.now only where there is no performance.now
fn step_clock() -> f64 {
    performance_now().unwrap_or_else(|_| js_sys::Date::now())
}

// a step of a sim, as SimReport
#[wasm_bindgen]
pub fn sim_step(sim: u32, req_str: String) -> Result<String, JsValue> {
    Ok(do_sim_step(sim, &req_str, step_clock)?)
}

fn do_sim_step(handle: u32, req_str: &str, now: impl Fn() -> f64) -> Result<String, String> {
    let req = serde_json::from_str::<SimStepRequest>(req_str).map_err(err_to_str)?;
    let deadline = match req.ms {
        Some(ms) if ms >= 0.0 && ms.is_finite() => Some(now() + ms),
        Some(ms) => return Err(format!("ms: {ms} is not a time to run")),
        None if req.iterations.is_none() => {
            return Err("sim_step: give iterations, ms, or both".into());
        }
        None => None,
    };
    let mut sims = SIMS.lock().map_err(err_to_str)?;
    let sim = sims.live.get_mut(&handle).ok_or_else(|| no_sim(handle))?;
    // the play to compare against, checked before any step
    let against = match &req.against {
        Some(sim_play) => {
            let (play, _) = sim.held(sim_play).map_err(|e| format!("against: {e}"))?;
            if !sim.candidates.iter().any(|c| c.play == play) {
                return Err("against: not in the sim".into());
            }
            Some(play)
        }
        None => None,
    };
    let prune = req.prune.unwrap_or(sim.parts == 1);
    let stepped = sim.step(req.iterations.unwrap_or(u64::MAX), prune, || {
        deadline.is_some_and(|deadline| now() >= deadline)
    })?;
    serde_json::to_string(&sim.report(stepped, against.as_ref())).map_err(err_to_str)
}

// what a change does to each play given
#[derive(Clone, Copy)]
enum SimChange {
    Add,
    Remove,
    Pause,
    Resume,
}

// a play a change was given: as the sim holds it, as analyze lists a play, and
// where it stood before the change ("absent" if not in the sim)
#[derive(serde::Serialize)]
struct SimChanged {
    play: kibitzer::JsonPlayWithEquity,
    was: &'static str,
}

// a change to a sim's candidates: every play given is checked first, and one
// that is no play here refuses the whole change, naming it; then each changes
// as it can, one that cannot left as it is
fn do_sim_change(handle: u32, plays_str: &str, change: SimChange) -> Result<String, String> {
    let sim_plays = serde_json::from_str::<Vec<SimPlay>>(plays_str).map_err(err_to_str)?;
    let mut sims = SIMS.lock().map_err(err_to_str)?;
    let sim = sims.live.get_mut(&handle).ok_or_else(|| no_sim(handle))?;
    let held = (sim_plays.iter().enumerate())
        .map(|(i, sim_play)| sim.held(sim_play).map_err(|e| format!("plays[{i}]: {e}")))
        .collect::<Result<Vec<_>, _>>()?;
    let mut answer = Vec::with_capacity(held.len());
    for (play, static_equity) in held {
        let at = sim.candidates.iter().position(|c| c.play == play);
        let was = at.map_or("absent", |at| sim.candidates[at].state.name());
        match (change, at) {
            (SimChange::Add, None) => sim.candidates.push(SimCandidate {
                play: play.clone(),
                static_equity,
                state: SimState::Running,
                first_iter: sim.iterations + 1,
                wins: Vec::new(),
                equities: Vec::new(),
                objective: stats::Stats::new(),
                equity: stats::Stats::new(),
                win: stats::Stats::new(),
                plies: (0..sim.plies).map(|_| SimPly::default()).collect(),
            }),
            (SimChange::Remove, Some(at)) => {
                sim.candidates.remove(at);
            }
            (SimChange::Pause, Some(at)) => sim.candidates[at].state = SimState::Paused,
            (SimChange::Resume, Some(at)) => sim.candidates[at].state = SimState::Running,
            _ => {}
        }
        answer.push(SimChanged {
            play: kibitzer::JsonPlayWithEquity {
                equity: static_equity,
                play: (&play).into(),
            },
            was,
        });
    }
    serde_json::to_string(&answer).map_err(err_to_str)
}

// plays, as a JSON array, made candidates, rolled out from the next iteration
// on; one already in the sim is left as it stands
#[wasm_bindgen]
pub fn sim_add(sim: u32, plays_str: String) -> Result<String, JsValue> {
    Ok(do_sim_change(sim, &plays_str, SimChange::Add)?)
}

// candidates taken out, their rollouts forgotten
#[wasm_bindgen]
pub fn sim_remove(sim: u32, plays_str: String) -> Result<String, JsValue> {
    Ok(do_sim_change(sim, &plays_str, SimChange::Remove)?)
}

// candidates kept, with their rollouts, but no longer rolled out
#[wasm_bindgen]
pub fn sim_pause(sim: u32, plays_str: String) -> Result<String, JsValue> {
    Ok(do_sim_change(sim, &plays_str, SimChange::Pause)?)
}

// paused or pruned candidates rolled out again from the next iteration on
#[wasm_bindgen]
pub fn sim_resume(sim: u32, plays_str: String) -> Result<String, JsValue> {
    Ok(do_sim_change(sim, &plays_str, SimChange::Resume)?)
}

// a sim let go, with the tables it holds
#[wasm_bindgen]
pub fn sim_free(sim: u32) -> Result<(), JsValue> {
    Ok(do_sim_free(sim)?)
}

fn do_sim_free(handle: u32) -> Result<(), String> {
    let mut sims = SIMS.lock().map_err(err_to_str)?;
    sims.live
        .remove(&handle)
        .map(|_| ())
        .ok_or_else(|| no_sim(handle))
}

// the live sims made with the table a name named, by handle
fn sims_holding(holds: impl Fn(&Sim) -> bool) -> Result<Vec<u32>, String> {
    let sims = SIMS.lock().map_err(err_to_str)?;
    let mut handles = (sims.live.iter())
        .filter(|(_, sim)| holds(sim))
        .map(|(&handle, _)| handle)
        .collect::<Vec<_>>();
    handles.sort_unstable();
    Ok(handles)
}

// the bytes allocated and not yet freed, which an endgame's memory cap reads: a
// wasm's memory only grows, so its size says nothing of what is held once a
// solve's table is freed
static HELD: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct Counting;

// every allocation counted onto the bytes held as it is made and off as it is
// freed
unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let p = unsafe { std::alloc::System.alloc(layout) };
        if !p.is_null() {
            HELD.fetch_add(layout.size(), std::sync::atomic::Ordering::Relaxed);
        }
        p
    }

    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        let p = unsafe { std::alloc::System.alloc_zeroed(layout) };
        if !p.is_null() {
            HELD.fetch_add(layout.size(), std::sync::atomic::Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(p, layout) };
        HELD.fetch_sub(layout.size(), std::sync::atomic::Ordering::Relaxed);
    }

    unsafe fn realloc(&self, p: *mut u8, layout: std::alloc::Layout, new_size: usize) -> *mut u8 {
        let q = unsafe { std::alloc::System.realloc(p, layout, new_size) };
        if !q.is_null() {
            HELD.fetch_add(new_size, std::sync::atomic::Ordering::Relaxed);
            HELD.fetch_sub(layout.size(), std::sync::atomic::Ordering::Relaxed);
        }
        q
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn held_bytes() -> usize {
    HELD.load(std::sync::atomic::Ordering::Relaxed)
}

// the page's clock as an endgame reads it: performance.now, as a sim's step's
fn page_clock() -> std::time::Duration {
    std::time::Duration::from_secs_f64(step_clock().max(0.0) / 1000.0)
}

// the memory the wasm may hold while an endgame solves, in MiB, unless its
// request says: a phone's browser may refuse a page much more
fn default_max_memory_mb() -> usize {
    256
}

// endgame_new: a position whose bag holds at most one tile (rack, board,
// lexicon, rules, each seat's score in points, the seat to move, and the
// scoreless turns and the passes in a row so far), and the most memory the wasm
// may hold while it solves
#[derive(serde::Deserialize)]
struct EndgameNewRequest {
    rack: Vec<u8>,
    #[serde(rename = "board")]
    board_tiles: Vec<Vec<i8>>,
    lexicon: String,
    rules: String,
    scores: Vec<i32>,
    turn: u8,
    zero_turns: u16,
    pass_turns: u16,
    #[serde(default = "default_max_memory_mb")]
    max_memory_mb: usize,
}

// endgame_step: at most this many milliseconds; 0 reads the last answer
#[derive(serde::Deserialize)]
struct EndgameStepRequest {
    ms: Option<f64>,
}

// a ply of an endgame's line: the play, and its mover's gain on the other from
// there to the game's end, in points
#[derive(serde::Serialize)]
struct EndgamePly {
    play: kibitzer::JsonPlay,
    value: f64,
}

// a tile the bag may hold: how many of it are unseen, and the play's gain if it
// is the one in the bag, in points
#[derive(serde::Serialize)]
struct EndgameBagTile {
    tile: u8,
    count: u32,
    value: f64,
}

// endgame_step's answer: the bag's tiles (0 or 1), the steps that solved,
// whether the answer is the endgame's own (the solve finished), the play to
// make (none before a step solved), its gain for the side to move from now to
// the game's end, and the lead it then ends with, in points; with an empty bag
// the depth searched and the line, with a tile in it the share of the tiles it
// may be that the play wins and the gain for each
#[derive(serde::Serialize)]
struct EndgameAnswer {
    bag: u8,
    steps: u64,
    exact: bool,
    play: Option<kibitzer::JsonPlay>,
    value: f64,
    spread: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<Vec<EndgamePly>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    win: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tiles: Option<Vec<EndgameBagTile>>,
}

// an endgame a caller steps and frees: its rules and its own word graph, of the
// lexicon's words its board can still take, as wolges's endgame command prunes
// its graph (each held here at one address, for the solver's table to be given
// back to, so the lexicon's name may be evicted while it lives), the position
// (the board, the side to move's rack, the tiles it cannot see with each one's
// count, whether one is in the bag, the seat to move, its lead in millipoints,
// and the passes and scoreless turns before it), the memory cap, the solver's
// table between steps, with a tile in the bag the search so far, and the last
// answer
struct Endgame {
    game_config: std::sync::Arc<game_config::GameConfig>,
    kwg: Box<Kwg>,
    arm_tables: Box<anagrams::ArmTables>,
    board_tiles: Vec<u8>,
    rack: Vec<u8>,
    unseen: Vec<u8>,
    in_bag: bool,
    turn: u8,
    lead: i32,
    pass_turns: u16,
    zero_turns: u16,
    max_memory: usize,
    table: Option<endgame::KeptTable>,
    peg: Option<endgame::PegProgress>,
    answer: EndgameAnswer,
}

impl EndgameAnswer {
    // whether an empty bag's answer gives way to a later step's: the step's is
    // exact, or deeper, or at its depth worth more (a play the step proved
    // better); a step starts its solve again on the kept table, and may stop
    // sooner than one before it did
    fn gives_way_to(&self, step: &EndgameAnswer) -> bool {
        self.play.is_none()
            || step.exact
            || step.depth > self.depth
            || (step.depth == self.depth && step.value > self.value)
    }

    // a later step's answer taken when this one gives way to it, always with a
    // tile in the bag (its search resumes, so its answer only deepens); the
    // depth never falls, as an exact answer may come from a shallower search
    // than one before it, the table holding what that one finished
    fn take(&mut self, step: EndgameAnswer, in_bag: bool) {
        if in_bag || self.gives_way_to(&step) {
            let depth = step.depth.max(self.depth);
            *self = EndgameAnswer { depth, ..step };
        } else {
            self.steps = step.steps;
        }
    }
}

// the endgames made, by handle; a handle is never used twice
#[derive(Default)]
struct Endgames {
    made: u32,
    live: fash::MyHashMap<u32, Endgame>,
}

static ENDGAMES: std::sync::LazyLock<std::sync::Mutex<Endgames>> =
    std::sync::LazyLock::new(Default::default);

fn no_endgame(handle: u32) -> String {
    format!("endgame {handle}: none such, or freed")
}

fn points(equity: equity::Equity) -> f64 {
    equity.raw() as f64 / equity::SCALE as f64
}

impl Endgame {
    // one solve of at most ms on the clock, on the table earlier steps kept
    fn solve(&mut self, ms: f64, clock: endgame::Clock) -> Result<(), String> {
        let mut solver = endgame::EndgameSolver::<kwg::Node24, kwg::Node24>::new(
            &self.game_config,
            &self.kwg,
            &self.arm_tables,
        )
        .map_err(err_to_str)?;
        if let Some(table) = self.table.take()
            && solver.give_table(table).is_err()
        {
            return Err("endgame: its table belongs to other tables".into());
        }
        solver.set_clock(clock);
        let limits = endgame::Limits {
            time: Some(std::time::Duration::from_secs_f64(ms / 1000.0)),
            states: None,
            memory: Some(endgame::MemoryLimit {
                bytes: self.max_memory,
                held: held_bytes,
            }),
        };
        let lead = points(equity::Equity::new(self.lead));
        let steps = self.answer.steps + 1;
        let answer = if let Some(progress) = self.peg.as_mut() {
            solver.set_scoreless_turns(self.pass_turns, self.zero_turns);
            let peg = solver.resume_peg_one_in_bag(progress, &limits);
            let peg = peg
                .map_err(err_to_str)
                .and_then(|peg| peg.map_err(|unsupported| unsupported.to_string()));
            let peg = match peg {
                Ok(peg) => peg,
                Err(e) => {
                    self.table = Some(solver.take_table());
                    return Err(e);
                }
            };
            // the search's margins are the leads the game ends with
            let value = points(peg.expected_margin) - lead;
            EndgameAnswer {
                bag: 1,
                steps,
                exact: peg.exact,
                play: peg.best_move.map(|best| match best {
                    endgame::PegMove::Pass => kibitzer::JsonPlay::Exchange {
                        tiles: Vec::new().into(),
                    },
                    endgame::PegMove::Place(play) => (&play).into(),
                }),
                value,
                spread: lead + value,
                depth: Some(peg.depth),
                line: None,
                win: Some(peg.win_pct as f64),
                tiles: Some(
                    (peg.hypotheses.iter())
                        .map(|&(tile, count, margin)| EndgameBagTile {
                            tile,
                            count,
                            value: points(margin) - lead,
                        })
                        .collect(),
                ),
            }
        } else {
            let mut racks = [Vec::new(), Vec::new()];
            racks[self.turn as usize].extend_from_slice(&self.rack);
            racks[1 - self.turn as usize].extend(
                (0u8..)
                    .zip(&self.unseen)
                    .flat_map(|(tile, &count)| std::iter::repeat_n(tile, count as usize)),
            );
            solver.init_after_scoreless_turns(
                &self.board_tiles,
                [&racks[0], &racks[1]],
                self.pass_turns,
                self.zero_turns,
            );
            let solved = match solver.solve_within(self.turn, &limits) {
                Ok(solved) => solved,
                Err(e) => {
                    self.table = Some(solver.take_table());
                    return Err(err_to_str(e));
                }
            };
            let mut line = Vec::new();
            solver.collect_pv(self.turn, &mut line);
            let value = points(solved.value);
            EndgameAnswer {
                bag: 0,
                steps,
                exact: solved.exact,
                play: line.first().map(|(_, play)| play.into()),
                value,
                spread: lead + value,
                depth: Some(solved.depth),
                line: Some(
                    (line.iter())
                        .map(|(value, play)| EndgamePly {
                            play: play.into(),
                            value: points(*value),
                        })
                        .collect(),
                ),
                win: None,
                tiles: None,
            }
        };
        self.table = Some(solver.take_table());
        self.answer.take(answer, self.in_bag);
        Ok(())
    }
}

// a new endgame's handle
#[wasm_bindgen]
pub fn endgame_new(req_str: String) -> Result<u32, JsValue> {
    Ok(do_endgame_new(&req_str)?)
}

fn do_endgame_new(req_str: &str) -> Result<u32, String> {
    let req = serde_json::from_str::<EndgameNewRequest>(req_str).map_err(err_to_str)?;
    let kwg = (CACHED_KWG
        .read()
        .map_err(err_to_str)?
        .get(&req.lexicon)
        .cloned())
    .ok_or("missing kwg")?;
    let game_config = (CACHED_GAME_CONFIG
        .read()
        .map_err(err_to_str)?
        .get(&req.rules)
        .cloned())
    .ok_or("missing game_config")?;
    if !kwg.fits_alphabet(game_config.alphabet().len()) {
        return Err(format!(
            "lexicon: has tiles the {:?} rules do not have",
            req.rules
        ));
    }
    if game_config.num_players() != 2 {
        return Err("endgame: the rules are not for two players".into());
    }
    let rack_size = game_config.rack_size() as usize;
    if req.rack.is_empty() {
        return Err("endgame: the rack is empty".into());
    }
    if req.rack.len() > rack_size {
        return Err(format!(
            "rack: max {rack_size} tiles, found {} tiles",
            req.rack.len()
        ));
    }
    let turn = req.turn as usize;
    if turn >= 2 {
        return Err(format!("turn: must be >= 0 and < 2, found {turn}"));
    }
    if req.scores.len() != 2 {
        return Err(format!(
            "scores: length must be 2, found {}",
            req.scores.len()
        ));
    }
    // a request's scores are points; the game keeps millipoints
    let scores = (req.scores.iter())
        .map(|&points| points.checked_mul(equity::SCALE))
        .collect::<Option<Vec<_>>>()
        .and_then(|scores| scores[turn].checked_sub(scores[1 - turn]))
        .ok_or_else(|| {
            format!(
                "scores: each must be within {} points, and their difference too, found {:?}",
                i32::MAX / equity::SCALE,
                req.scores
            )
        })?;
    if req.max_memory_mb == 0 || req.max_memory_mb > 4096 {
        return Err(format!(
            "max_memory_mb: 1 to 4096, found {}",
            req.max_memory_mb
        ));
    }

    let mut kibitzer = kibitzer::Kibitzer::new();
    kibitzer
        .prepare(&game_config, &req.rack, &req.board_tiles)
        .map_err(err_to_str)?;
    // the tiles the side to move cannot see: the other side's rack, and the
    // bag's, if any
    let unseen = kibitzer.available_tally.clone();
    let num_unseen = unseen.iter().map(|&count| count as usize).sum::<usize>();
    // the other side holding no tiles went out, so the game is over
    if num_unseen == 0 {
        return Err("endgame: the other side holds no tiles, so the game is over".into());
    }
    let in_bag = num_unseen > rack_size;
    if num_unseen > rack_size + 1 {
        return Err(format!(
            "endgame: the bag holds {} tiles; at most one is solved",
            num_unseen - rack_size
        ));
    }
    if in_bag && req.rack.len() != rack_size {
        return Err("endgame: with a tile in the bag the rack must be full".into());
    }
    let mut rack = req.rack;
    rack.sort_unstable();
    // the lexicon's words the board can still take, whatever the racks, made
    // into the endgame's own graph, as wolges's endgame command prunes its own:
    // the same answers, a far smaller graph to generate on
    let mut words = fash::MyHashSet::<bites::Bites>::default();
    movegen::KurniaMoveGenerator::new(&game_config).gen_remaining_words(
        &movegen::BoardSnapshot {
            board_tiles: &kibitzer.board_tiles,
            game_config: &game_config,
            kwg: &kwg.kwg,
            anagrams: None,
            rack_lengths: None,
            klv: &Klv::empty().map_err(err_to_str)?,
        },
        |word: &[u8]| {
            words.insert(word.into());
        },
    );
    let mut words = words.into_iter().collect::<Vec<_>>();
    words.sort_unstable();
    let bytes = match game_config.game_rules() {
        game_config::GameRules::Classic => build::build_big(
            build::BuildContent::Gaddawg,
            build::BuildLayout::Wolges,
            build::BuildOrder::Sorted,
            &words,
        ),
        game_config::GameRules::Jumbled => build::build_big(
            build::BuildContent::DawgOnly,
            build::BuildLayout::Wolges,
            build::BuildOrder::Sorted,
            &build::make_alphagrams(&words),
        ),
    }
    .map_err(err_to_str)?;
    let peg = in_bag.then(|| {
        endgame::PegProgress::new(
            req.turn,
            &kibitzer.board_tiles,
            &rack,
            &unseen,
            equity::Equity::new(scores),
        )
    });
    let endgame = Endgame {
        game_config,
        kwg: Box::new(Kwg::from_bytes_alloc(&bytes).map_err(err_to_str)?),
        arm_tables: Box::new(anagrams::ArmTables::None),
        board_tiles: kibitzer.board_tiles.clone(),
        rack,
        unseen,
        in_bag,
        turn: req.turn,
        lead: scores,
        pass_turns: req.pass_turns,
        zero_turns: req.zero_turns,
        max_memory: req.max_memory_mb << 20,
        table: None,
        peg,
        answer: EndgameAnswer {
            bag: in_bag as u8,
            steps: 0,
            exact: false,
            play: None,
            value: 0.0,
            spread: points(equity::Equity::new(scores)),
            depth: Some(0),
            line: (!in_bag).then(Vec::new),
            win: in_bag.then_some(0.0),
            tiles: in_bag.then(Vec::new),
        },
    };

    let mut endgames = ENDGAMES.lock().map_err(err_to_str)?;
    let handle = endgames
        .made
        .checked_add(1)
        .ok_or("too many endgames made")?;
    endgames.made = handle;
    endgames.live.insert(handle, endgame);
    Ok(handle)
}

// a step of an endgame, as EndgameAnswer
#[wasm_bindgen]
pub fn endgame_step(handle: u32, req_str: String) -> Result<String, JsValue> {
    Ok(do_endgame_step(handle, &req_str, page_clock)?)
}

fn do_endgame_step(handle: u32, req_str: &str, clock: endgame::Clock) -> Result<String, String> {
    let req = serde_json::from_str::<EndgameStepRequest>(req_str).map_err(err_to_str)?;
    let ms = match req.ms {
        Some(ms) if ms >= 0.0 && ms.is_finite() => ms,
        Some(ms) => return Err(format!("ms: {ms} is not a time to run")),
        None => return Err("endgame_step: give ms".into()),
    };
    let mut endgames = ENDGAMES.lock().map_err(err_to_str)?;
    let endgame = endgames
        .live
        .get_mut(&handle)
        .ok_or_else(|| no_endgame(handle))?;
    // an exact answer stays: no solve can better it
    if ms > 0.0 && !endgame.answer.exact {
        endgame.solve(ms, clock)?;
    }
    serde_json::to_string(&endgame.answer).map_err(err_to_str)
}

// an endgame let go, with its table and the tables it holds
#[wasm_bindgen]
pub fn endgame_free(handle: u32) -> Result<(), JsValue> {
    Ok(do_endgame_free(handle)?)
}

fn do_endgame_free(handle: u32) -> Result<(), String> {
    let mut endgames = ENDGAMES.lock().map_err(err_to_str)?;
    endgames
        .live
        .remove(&handle)
        .map(|_| ())
        .ok_or_else(|| no_endgame(handle))
}

#[cfg(test)]
mod tests {
    use super::*;

    // the rules are wolges's own, fixed when the wasm is built, so the
    // reckoning of each one's best play that a game made at run time needs is
    // done here, not as a table loads: each must leave a play's score the half
    // of an equity LEAVE_LIMIT does not take
    #[test]
    fn every_rules_takes_leaves_up_to_the_limit() {
        do_this_on_startup().unwrap();
        let rules = CACHED_GAME_CONFIG.read().unwrap();
        assert!(!rules.is_empty());
        for (name, game_config) in rules.iter() {
            assert!(
                game_config
                    .check_leaves((-LEAVE_LIMIT, LEAVE_LIMIT))
                    .is_ok(),
                "{name}"
            );
        }
    }

    // english tiles
    const A: u8 = 1;
    const C: u8 = 3;
    const E: u8 = 5;
    const S: u8 = 19;
    const T: u8 = 20;

    // a lexicon of a few words cached under this name, beside the rules and the
    // empty leaves startup caches
    fn tiny_lexicon(name: &str) {
        tiny_lexicon_for(name, false);
    }

    // the same, for jumbled rules as well: a graph of the words' alphagrams
    fn tiny_lexicon_for(name: &str, jumbled: bool) {
        do_this_on_startup().unwrap();
        let mut words = [
            &[A, T][..],
            &[T, A],
            &[A, S],
            &[E, S],
            &[E, T],
            &[C, A, T],
            &[A, C, T],
            &[C, A, T, S],
            &[S, C, A, T],
            &[A, C, T, S],
            &[E, A, T],
            &[A, T, E],
            &[T, E, A],
            &[E, T, A],
            &[E, A, T, S],
            &[S, E, A, T],
            &[T, E, A, S],
            &[S, A, T],
            &[S, E, T],
        ]
        .iter()
        .map(|&word| word.into())
        .collect::<Vec<bites::Bites>>();
        words.sort_unstable();
        let bytes = if jumbled {
            build::build(
                build::BuildContent::DawgOnly,
                build::BuildLayout::Wolges,
                build::BuildOrder::Sorted,
                &build::make_alphagrams(&words),
            )
        } else {
            build::build(
                build::BuildContent::Gaddawg,
                build::BuildLayout::Wolges,
                build::BuildOrder::Sorted,
                &words,
            )
        }
        .unwrap();
        let graph = Kwg::from_reader::<kwg::Node22>(bytes.len(), |at, piece| {
            piece.copy_from_slice(&bytes[at..at + piece.len()])
        })
        .unwrap();
        CACHED_KWG
            .write()
            .unwrap()
            .insert(name.into(), CachedKwg::new(graph).into());
    }

    // an empty board but for CAT across row 7 from column 6, as json
    fn cat_board() -> String {
        let mut board = vec![vec![0i8; 15]; 15];
        board[7][6] = C as i8;
        board[7][7] = A as i8;
        board[7][8] = T as i8;
        serde_json::to_string(&board).unwrap()
    }

    #[test]
    fn hooks_give_the_letters_that_fit_every_empty_square_beside_a_tile() {
        tiny_lexicon("tiny-hooks");
        let req = format!(
            r#"{{"board":{},"lexicon":"tiny-hooks","rules":"CrosswordGame"}}"#,
            cat_board()
        );
        let answer = serde_json::from_str::<serde_json::Value>(&do_hooks(&req).unwrap()).unwrap();
        // every empty square beside CAT, row by row. Above C nothing makes a
        // word down; above A, T (ta); above T, A or E (at, et); left of CAT, S
        // (scat); right of it, S (cats); below C nothing; below A, S or T (as,
        // at); below T, A (ta)
        assert_eq!(
            answer,
            serde_json::json!({"squares": [
                {"row": 6, "col": 6, "across": null, "down": []},
                {"row": 6, "col": 7, "across": null, "down": [T]},
                {"row": 6, "col": 8, "across": null, "down": [A, E]},
                {"row": 7, "col": 5, "across": [S], "down": null},
                {"row": 7, "col": 9, "across": [S], "down": null},
                {"row": 8, "col": 6, "across": null, "down": []},
                {"row": 8, "col": 7, "across": null, "down": [S, T]},
                {"row": 8, "col": 8, "across": null, "down": [A]},
            ]})
        );
    }

    // a sim of this board, seed 7, scores level, with what more is given
    fn sim_of(lexicon: &str, board: &str, more: &str) -> u32 {
        tiny_lexicon(lexicon);
        do_sim_new(&format!(
            r#"{{"rack":[{S},{E},{A},{T}],"board":{board},"lexicon":"{lexicon}","leave":"noleave","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0,"seed":7{more}}}"#
        ))
        .unwrap()
    }

    // S hooked onto CAT, making CATS
    const CATS: &str = r#"{"action":"play","down":false,"lane":7,"idx":6,"word":[0,0,0,19]}"#;
    // a pass
    const PASS: &str = r#"{"action":"exchange","tiles":[]}"#;
    // A above T, making AT down
    const AT: &str = r#"{"action":"play","down":true,"lane":8,"idx":6,"word":[1,0]}"#;
    // S before CAT, making SCAT
    const SCAT: &str = r#"{"action":"play","down":false,"lane":7,"idx":5,"word":[19,0,0,0]}"#;

    // a sim of CAT's board with CATS and a pass as its candidates
    fn cat_sim(lexicon: &str) -> u32 {
        let sim = sim_of(lexicon, &cat_board(), "");
        do_sim_add(sim, &format!("[{CATS},{PASS}]")).unwrap();
        sim
    }

    fn report(answer: &str) -> serde_json::Value {
        serde_json::from_str(answer).unwrap()
    }

    fn step(sim: u32, req: &str) -> serde_json::Value {
        report(&do_sim_step(sim, req, || 0.0).unwrap())
    }

    fn do_sim_add(handle: u32, plays: &str) -> Result<String, String> {
        do_sim_change(handle, plays, SimChange::Add)
    }

    fn states(answer: &serde_json::Value) -> Vec<String> {
        (answer["candidates"].as_array().unwrap().iter())
            .map(|c| c["state"].as_str().unwrap().to_string())
            .collect()
    }

    fn ns(answer: &serde_json::Value) -> Vec<u64> {
        (answer["candidates"].as_array().unwrap().iter())
            .map(|c| c["n"].as_u64().unwrap())
            .collect()
    }

    #[test]
    fn a_sim_stepped_in_parts_is_the_sim_stepped_at_once() {
        let a = cat_sim("tiny-sim-a");
        let b = cat_sim("tiny-sim-b");
        step(a, r#"{"iterations":3,"prune":false}"#);
        let in_parts = step(a, r#"{"iterations":2,"prune":false}"#);
        let at_once = step(b, r#"{"iterations":5,"prune":false}"#);
        assert_eq!(in_parts["iterations"], 5);
        assert_eq!(in_parts["stepped"], 2);
        assert_eq!(at_once["stepped"], 5);
        assert_eq!(in_parts["candidates"], at_once["candidates"]);
        let first = &at_once["candidates"][0];
        assert_eq!(first["n"], 5);
        assert_eq!(first["state"], "running");
        assert_eq!(first["play"]["word"], serde_json::json!([0, 0, 0, S]));
        // the hooked S scores C, A, T, and S: 3, 1, 1, and 1
        assert_eq!(first["play"]["score"], 6);
        // the plies wolges's own sims play by default, each reached by every
        // rollout here, none taking a whole rack: the tiny lexicon's words are
        // four tiles at most
        let plies = first["plies"].as_array().unwrap();
        assert_eq!(plies.len(), move_picker::DEFAULT_NUM_SIM_PLIES);
        for ply in plies {
            assert_eq!(ply["n"], 5);
            assert_eq!(ply["bingo"], 0.0);
        }

        // iterations 0 reads it as it is
        let read = step(b, r#"{"iterations":0}"#);
        assert_eq!(
            (read["iterations"].clone(), read["stepped"].clone()),
            (5.into(), 0.into())
        );
        assert_eq!(read["candidates"], at_once["candidates"]);
        do_sim_free(a).unwrap();
        do_sim_free(b).unwrap();
    }

    // what a candidate's rollouts come to depends on the seed and the iteration
    // alone, never on the other candidates: so workers may share one sim's
    // candidates out among them
    #[test]
    fn a_candidates_rollouts_are_the_same_whatever_shares_its_sim() {
        let all = cat_sim("tiny-sim-share");
        do_sim_add(all, &format!("[{AT}]")).unwrap();
        let alone = sim_of("tiny-sim-share", &cat_board(), "");
        do_sim_add(alone, &format!("[{AT}]")).unwrap();
        let all = step(all, r#"{"iterations":20,"prune":false}"#);
        let alone = step(alone, r#"{"iterations":20,"prune":false}"#);
        assert_eq!(all["candidates"][2], alone["candidates"][0]);
    }

    // a sim run in parts, pooled, is the sim run whole: the same iterations, so
    // the same n and, but for rounding, the same statistics
    #[test]
    fn a_sims_parts_together_are_the_whole_sim() {
        let whole = sim_of("tiny-sim-parts", &cat_board(), "");
        let parts = (0..3)
            .map(|part| {
                sim_of(
                    "tiny-sim-parts",
                    &cat_board(),
                    &format!(r#","part":{part},"parts":3"#),
                )
            })
            .collect::<Vec<_>>();
        for &sim in std::iter::once(&whole).chain(&parts) {
            do_sim_add(sim, &format!("[{CATS},{PASS},{AT}]")).unwrap();
        }
        let whole = step(whole, r#"{"iterations":12,"prune":false}"#);
        // a part prunes only when asked, so these steps run every candidate
        let parts = (parts.iter())
            .map(|&sim| step(sim, r#"{"iterations":4}"#))
            .collect::<Vec<_>>();
        // the parts' statistics pooled: n summed, the mean n-weighted, and the
        // sum of squares each part's own plus its mean's distance from the whole
        let pooled = |c: usize, stat: &str| {
            let parts = (parts.iter())
                .map(|part| {
                    let one = &part["candidates"][c];
                    (
                        one["n"].as_f64().unwrap(),
                        one[stat]["mean"].as_f64().unwrap(),
                        one[stat]["variance"].as_f64().unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            let n = parts.iter().map(|p| p.0).sum::<f64>();
            let mean = parts.iter().map(|p| p.0 * p.1).sum::<f64>() / n;
            let sum_of_squares = (parts.iter())
                .map(|p| p.2 * (p.0 - 1.0) + p.0 * (p.1 - mean).powi(2))
                .sum::<f64>();
            (n, mean, sum_of_squares / (n - 1.0))
        };
        for c in 0..3 {
            let one = &whole["candidates"][c];
            assert_eq!(one["n"], 12);
            for stat in ["win", "equity", "objective"] {
                let (n, mean, variance) = pooled(c, stat);
                assert_eq!(n, 12.0);
                assert!(
                    (mean - one[stat]["mean"].as_f64().unwrap()).abs() < 1e-9,
                    "{c} {stat}"
                );
                assert!(
                    (variance - one[stat]["variance"].as_f64().unwrap()).abs() < 1e-6,
                    "{c} {stat}"
                );
            }
        }
        // each part numbers its iterations apart from the others
        assert_ne!(parts[0]["candidates"], parts[1]["candidates"]);
    }

    #[test]
    fn a_sim_steps_until_its_time_is_up() {
        let sim = cat_sim("tiny-sim-clock");
        // the clock reads 0, 1, 2, ... a call: the deadline is 0 + 3.5, and the
        // reads before iterations read 1, 2, 3, then 4
        let clock = std::cell::Cell::new(0.0);
        let now = || {
            let t = clock.get();
            clock.set(t + 1.0);
            t
        };
        let answer = report(&do_sim_step(sim, r#"{"ms":3.5}"#, now).unwrap());
        assert_eq!(answer["stepped"], 3);
        // iterations bound it too, whichever ends first
        let answer = report(&do_sim_step(sim, r#"{"ms":1000,"iterations":2}"#, now).unwrap());
        assert_eq!(answer["stepped"], 2);
        assert!(do_sim_step(sim, r#"{}"#, now).is_err());
        assert!(do_sim_step(sim, r#"{"ms":-1}"#, now).is_err());
        do_sim_free(sim).unwrap();
    }

    // scores are points: a thousand of them ahead wins, behind loses; and a sim
    // must be told them
    #[test]
    fn a_sims_scores_are_points() {
        tiny_lexicon("tiny-sim-scores");
        let sim_new = |scores: &str| {
            do_sim_new(&format!(
                r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-sim-scores","leave":"noleave","rules":"CrosswordGame","turn":0,"zero_turns":0,"pass_turns":0,"seed":7{scores}}}"#,
                cat_board()
            ))
        };
        for (scores, ahead) in [
            (r#","scores":[1000,0]"#, true),
            (r#","scores":[0,1000]"#, false),
        ] {
            let sim = sim_new(scores).unwrap();
            do_sim_add(sim, &format!("[{PASS}]")).unwrap();
            let answer = step(sim, r#"{"iterations":4}"#);
            let win = answer["candidates"][0]["win"]["mean"].as_f64().unwrap();
            assert!(
                if ahead { win > 0.999 } else { win < 0.001 },
                "{scores}: {win}"
            );
            do_sim_free(sim).unwrap();
        }
        // past what millipoints can hold, or left out: refused
        assert!(sim_new(r#","scores":[2147484,0]"#).is_err());
        assert!(sim_new("").unwrap_err().contains("scores"));
        // the seat to move, the scoreless turns, and the passes so far are the
        // position's too: left out, refused
        for field in ["turn", "zero_turns", "pass_turns"] {
            let req = format!(
                r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-sim-scores","leave":"noleave","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0}}"#,
                cat_board()
            )
            .replace(&format!(r#","{field}":0"#), "");
            let err = do_sim_new(&req).unwrap_err();
            assert!(err.contains(&format!("missing field `{field}`")), "{err}");
        }
    }

    // a sim knows the passes in a row so far: under German rules, where four
    // passes in a row end the game, after three a pass ends it at once, and no
    // ply follows
    #[test]
    fn a_sims_passes_so_far_can_end_its_game() {
        tiny_lexicon("tiny-sim-passes");
        let plies_reached = |pass_turns: u16| {
            let sim = do_sim_new(&format!(
                r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-sim-passes","leave":"noleave","rules":"CrosswordGame/german","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":{pass_turns},"seed":7}}"#,
                cat_board()
            ))
            .unwrap();
            do_sim_add(sim, &format!("[{PASS}]")).unwrap();
            let answer = step(sim, r#"{"iterations":4,"prune":false}"#);
            do_sim_free(sim).unwrap();
            answer["candidates"][0]["plies"][0]["n"].as_u64().unwrap()
        };
        assert_eq!(plies_reached(0), 4);
        assert_eq!(plies_reached(3), 0);
    }

    // near the end the others are dealt what is left: here the two blanks, the
    // board holding every other tile but the rack
    #[test]
    fn a_sims_others_hold_what_is_left_by_default() {
        tiny_lexicon("tiny-sim-late");
        let Tables { game_config, .. } =
            tables("tiny-sim-late", "noleave", "CrosswordGame").unwrap();
        let alphabet = game_config.alphabet();
        let mut left = (0..alphabet.len())
            .map(|tile| alphabet.freq(tile))
            .collect::<Vec<_>>();
        for tile in [S, E, A, T] {
            left[tile as usize] -= 1;
        }
        let mut board = vec![vec![0i8; 15]; 15];
        let mut squares = board.iter_mut().flatten();
        for (tile, &count) in left.iter().enumerate().skip(1) {
            for _ in 0..count {
                *squares.next().unwrap() = tile as i8;
            }
        }
        let sim = sim_of("tiny-sim-late", &serde_json::to_string(&board).unwrap(), "");
        do_sim_add(sim, &format!("[{PASS}]")).unwrap();
        let answer = step(sim, r#"{"iterations":2}"#);
        assert_eq!(answer["stepped"], 2);
        do_sim_free(sim).unwrap();
    }

    // the tiles known to be on the opponent's rack are on it every iteration
    #[test]
    fn a_sims_known_tiles_are_on_the_opponents_rack() {
        let handle = sim_of(
            "tiny-sim-known",
            &cat_board(),
            &format!(r#","known":[{E},{E},0]"#),
        );
        {
            let mut sims = SIMS.lock().unwrap();
            let sim = sims.live.get_mut(&handle).unwrap();
            for iteration in 1..=20 {
                sim.simmer.restore_prepared(&sim.pristine);
                sim.simmer.reseed(iteration_seed(sim.seed, iteration));
                sim.simmer.prepare_iteration();
                let rack = &sim.simmer.prepared_state().players[1].rack;
                assert_eq!(rack.len(), 7);
                for (tile, count) in [(E, 2), (0, 1)] {
                    assert!(
                        rack.iter().filter(|&&t| t == tile).count() >= count,
                        "{rack:?}"
                    );
                }
            }
        }
        do_sim_free(handle).unwrap();
        // more than the opponent holds, or more than are unseen: refused
        tiny_lexicon("tiny-sim-known-bad");
        for known in ["[5,5,5,5,5,5,5,5]", "[0,0,0]"] {
            let err = do_sim_new(&format!(
                r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-sim-known-bad","leave":"noleave","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0,"known":{known}}}"#,
                cat_board()
            ))
            .unwrap_err();
            assert!(err.starts_with("known: "), "{err}");
        }
    }

    #[test]
    fn candidates_join_pause_resume_and_leave_between_steps() {
        let sim = cat_sim("tiny-sim-change");
        step(sim, r#"{"iterations":4,"prune":false}"#);
        // AT joins; CATS is in already, and stays as it stands
        let answer = report(&do_sim_add(sim, &format!("[{AT},{CATS}]")).unwrap());
        assert_eq!(answer[0]["was"], "absent");
        assert_eq!(answer[1]["was"], "running");
        // a play as analyze lists one: its score recounted, its static equity
        assert_eq!(answer[1]["play"]["score"], 6);
        assert!(answer[1]["play"]["equity"].is_number());
        assert_eq!(
            ns(&step(sim, r#"{"iterations":1,"prune":false}"#)),
            [5, 5, 1]
        );

        // a play the lexicon lacks, or one the rack cannot make, refuses the
        // whole change, naming it, and nothing joins
        let te = r#"{"action":"play","down":true,"lane":8,"idx":7,"word":[0,5]}"#;
        let err = do_sim_add(sim, &format!("[{SCAT},{te}]")).unwrap_err();
        assert!(
            err.starts_with("plays[1]: ") && err.contains("lacks"),
            "{err}"
        );
        let off = r#"{"action":"play","down":true,"lane":8,"idx":6,"word":[26,0]}"#;
        let err = do_sim_add(sim, &format!("[{off}]")).unwrap_err();
        assert!(err.starts_with("plays[0]: "), "{err}");
        assert_eq!(ns(&step(sim, r#"{"iterations":0}"#)), [5, 5, 1]);

        // paused, a candidate keeps its rollouts and runs no more; resumed, it
        // runs again from the next iteration
        let answer = report(&do_sim_change(sim, &format!("[{PASS}]"), SimChange::Pause).unwrap());
        assert_eq!(answer[0]["was"], "running");
        let stepped = step(sim, r#"{"iterations":3,"prune":false}"#);
        assert_eq!(ns(&stepped), [8, 5, 4]);
        assert_eq!(states(&stepped), ["running", "paused", "running"]);
        do_sim_change(sim, &format!("[{PASS}]"), SimChange::Resume).unwrap();
        assert_eq!(
            ns(&step(sim, r#"{"iterations":1,"prune":false}"#)),
            [9, 6, 5]
        );

        // removed, a candidate's rollouts are forgotten, and added again it
        // starts anew; one not in the sim is left as it is
        let answer =
            report(&do_sim_change(sim, &format!("[{PASS},{PASS}]"), SimChange::Remove).unwrap());
        assert_eq!(
            (answer[0]["was"].clone(), answer[1]["was"].clone()),
            ("running".into(), "absent".into())
        );
        do_sim_add(sim, &format!("[{PASS}]")).unwrap();
        assert_eq!(
            ns(&step(sim, r#"{"iterations":1,"prune":false}"#)),
            [10, 6, 1]
        );
        do_sim_free(sim).unwrap();
    }

    // a one-tile play is one candidate whichever way it is written: held across
    // where its tile makes an across word, as wolges's scorer holds it
    #[test]
    fn a_one_tile_play_is_one_candidate_whichever_way_it_is_written() {
        // CAT, and an A below its C: a T right of that A makes AT across and,
        // under CAT's A, AT down
        let mut board = vec![vec![0i8; 15]; 15];
        board[7][6] = C as i8;
        board[7][7] = A as i8;
        board[7][8] = T as i8;
        board[8][6] = A as i8;
        let sim = sim_of(
            "tiny-sim-one-tile",
            &serde_json::to_string(&board).unwrap(),
            "",
        );
        let down = format!(r#"{{"action":"play","down":true,"lane":7,"idx":7,"word":[0,{T}]}}"#);
        let across = format!(r#"{{"action":"play","down":false,"lane":8,"idx":6,"word":[0,{T}]}}"#);
        let answer = report(&do_sim_add(sim, &format!("[{down},{across}]")).unwrap());
        assert_eq!(
            (answer[0]["was"].clone(), answer[1]["was"].clone()),
            ("absent".into(), "running".into())
        );
        assert_eq!(answer[0]["play"], answer[1]["play"]);
        assert_eq!(answer[0]["play"]["down"], false);
        do_sim_free(sim).unwrap();
    }

    // a candidate behind the leader on the draws they share is pruned at a look
    // and stepped no more; without pruning, every candidate runs on
    #[test]
    fn a_sim_prunes_what_is_behind_the_leader() {
        // the pass keeps the S that hooks CATS later, so it is behind by under
        // two points a draw, so by little in the chance to win, and it takes a
        // while to tell
        let sim = cat_sim("tiny-sim-prune");
        let answer = step(sim, r#"{"iterations":256}"#);
        assert_eq!(states(&answer), ["running", "pruned"]);
        let n = ns(&answer);
        assert_eq!(n[0], 256);
        assert!(
            n[1] < 256 && n[1].is_multiple_of(move_picker::PRUNE_CADENCE),
            "{n:?}"
        );
        // a pruned candidate is resumed as a paused one is
        do_sim_change(sim, &format!("[{PASS}]"), SimChange::Resume).unwrap();
        assert_eq!(
            states(&step(sim, r#"{"iterations":1,"prune":false}"#)),
            ["running", "running"]
        );
        do_sim_free(sim).unwrap();
        let sim = cat_sim("tiny-sim-prune");
        let answer = step(sim, r#"{"iterations":64,"prune":false}"#);
        assert_eq!(states(&answer), ["running", "running"]);
        assert_eq!(ns(&answer), [64, 64]);
        // with nothing running, a step steps nothing
        do_sim_change(sim, &format!("[{CATS},{PASS}]"), SimChange::Pause).unwrap();
        assert_eq!(step(sim, r#"{"iterations":5}"#)["stepped"], 0);
        do_sim_free(sim).unwrap();
    }

    // a running candidate whose rollouts, from the first iteration, had these
    // chances to win and equities
    fn rolled(wins: &[f64], equities: &[f64]) -> SimCandidate {
        let mut candidate = SimCandidate {
            play: SimPlay::Exchange {
                tiles: Box::new([]),
            }
            .to_play(),
            static_equity: 0.0,
            state: SimState::Running,
            first_iter: 1,
            wins: wins.into(),
            equities: equities.into(),
            objective: stats::Stats::new(),
            equity: stats::Stats::new(),
            win: stats::Stats::new(),
            plies: Vec::new(),
        };
        wins.iter().for_each(|&win| candidate.win.update(win));
        (equities.iter()).for_each(|&equity| candidate.equity.update(equity));
        candidate
    }

    // a sim prunes by the chance to win, the leader the play a page sorting by
    // it lists first; but a candidate whose game is decided as the leader's is,
    // both won or both lost, by equity
    #[test]
    fn a_sim_prunes_by_the_chance_to_win_and_decided_games_by_equity() {
        // decided: within one game, in a whole default sim's, of certain
        assert_eq!(outcome(1.0 - DECIDED), Outcome::Won);
        assert_eq!(outcome(DECIDED), Outcome::Lost);
        assert_eq!(outcome(1.0 - 2.0 * DECIDED), Outcome::Open);
        assert_eq!(outcome(2.0 * DECIDED), Outcome::Open);
        assert!(by_equity(1.0 - DECIDED, 1.0) && by_equity(0.0, DECIDED));
        assert!(!by_equity(0.6, 1.0) && !by_equity(0.0, 1.0) && !by_equity(0.5, 0.5));

        let z = move_picker::paired_z(move_picker::DEFAULT_NUM_SIM_ITERS);
        // 32 iterations alternating between two values
        let two = |a: f64, b: f64| (0..32).map(|i| [a, b][i % 2]).collect::<Vec<_>>();
        let standing = |candidates: &[SimCandidate]| {
            (candidates.iter())
                .map(|c| c.state.name())
                .collect::<Vec<_>>()
        };
        // two won games and an open one: the leader is the won one of the
        // higher equity, the other won one is behind it in equity, and the open
        // one, though its equity is the highest, is behind it in the chance to
        // win
        let mut candidates = [
            rolled(&two(1.0, 1.0), &two(20.0, 22.0)),
            rolled(&two(1.0, 1.0), &two(30.0, 33.0)),
            rolled(&two(0.5, 0.7), &two(90.0, 95.0)),
        ];
        assert!(std::ptr::eq(leader(&candidates).unwrap(), &candidates[1]));
        prune_behind(&mut candidates, z);
        assert_eq!(standing(&candidates), ["pruned", "running", "pruned"]);
        // every game lost: the leader is the one of the highest equity, and the
        // others are behind it in equity
        let mut candidates = [
            rolled(&two(0.0, 0.0), &two(-90.0, -95.0)),
            rolled(&two(0.0, 0.0), &two(-20.0, -22.0)),
        ];
        prune_behind(&mut candidates, z);
        assert_eq!(standing(&candidates), ["pruned", "running"]);
        // open games: the leader is the one of the highest chance to win, and a
        // play of more equity but less chance to win is behind it; a play never
        // apart from it in the chance to win is not
        let mut candidates = [
            rolled(&two(0.5, 0.7), &two(90.0, 95.0)),
            rolled(&two(0.6, 0.8), &two(10.0, 12.0)),
            rolled(&two(0.6, 0.8), &two(0.0, 1.0)),
        ];
        prune_behind(&mut candidates, z);
        assert_eq!(standing(&candidates), ["pruned", "running", "running"]);
        // a paused candidate is not the leader, and is not pruned
        let mut candidates = [
            rolled(&two(0.6, 0.8), &two(10.0, 12.0)),
            rolled(&two(0.9, 0.9), &two(10.0, 12.0)),
        ];
        candidates[1].state = SimState::Paused;
        prune_behind(&mut candidates, z);
        assert_eq!(standing(&candidates), ["running", "paused"]);

        // a sim far ahead: every rollout's game is won, so CATS and the pass are
        // compared by equity, and the pass, which scores nothing now, is pruned
        tiny_lexicon("tiny-sim-decided");
        let sim = do_sim_new(&format!(
            r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-sim-decided","leave":"noleave","rules":"CrosswordGame","scores":[1000,0],"turn":0,"zero_turns":0,"pass_turns":0,"seed":7}}"#,
            cat_board()
        ))
        .unwrap();
        do_sim_add(sim, &format!("[{CATS},{PASS}]")).unwrap();
        let answer = step(sim, r#"{"iterations":256}"#);
        assert_eq!(states(&answer), ["running", "pruned"]);
        for c in 0..2 {
            let win = answer["candidates"][c]["win"]["mean"].as_f64().unwrap();
            assert_eq!(outcome(win), Outcome::Won, "{win}");
        }
        do_sim_free(sim).unwrap();
    }

    // each candidate against a play: its chance to win, equity, and objective
    // less that play's, on the iterations both ran
    #[test]
    fn a_sim_compares_its_candidates_against_a_play() {
        let sim = cat_sim("tiny-sim-against");
        step(sim, r#"{"iterations":6,"prune":false}"#);
        do_sim_add(sim, &format!("[{AT}]")).unwrap();
        let answer = step(
            sim,
            &format!(r#"{{"iterations":4,"prune":false,"against":{CATS}}}"#),
        );
        let against = |c: usize, stat: &str| answer["candidates"][c]["against"][stat].clone();
        for stat in ["win", "equity", "objective"] {
            assert_eq!(
                against(0, stat),
                serde_json::json!({"n": 10, "mean": 0.0, "variance": 0.0})
            );
            assert_eq!(against(1, stat)["n"], 10);
            assert_eq!(against(2, stat)["n"], 4);
            assert!(against(1, stat)["mean"].as_f64().unwrap() < 0.0, "{stat}");
            // the pass and CATS ran the same iterations: the mean of the
            // difference is the difference of the means, the objective's as the
            // rollouts made it
            let mean = |c: usize| answer["candidates"][c][stat]["mean"].as_f64().unwrap();
            assert!(
                (against(1, stat)["mean"].as_f64().unwrap() - (mean(1) - mean(0))).abs() < 1e-9,
                "{stat}"
            );
        }
        // without one, none
        assert!(
            step(sim, r#"{"iterations":0}"#)["candidates"][0]
                .get("against")
                .is_none()
        );
        // a play not in the sim: refused before any step
        let err = do_sim_step(
            sim,
            &format!(r#"{{"iterations":3,"against":{SCAT}}}"#),
            || 0.0,
        )
        .unwrap_err();
        assert_eq!(err, "against: not in the sim");
        assert_eq!(step(sim, r#"{"iterations":0}"#)["iterations"], 10);
        do_sim_free(sim).unwrap();
    }

    #[test]
    fn a_freed_sim_is_gone_and_its_tables_can_go() {
        let sim = cat_sim("tiny-sim-free");
        // while the sim lives, evict_kwg refuses the lexicon's name
        assert_eq!(
            sims_holding(|s| s.lexicon == "tiny-sim-free").unwrap(),
            [sim]
        );
        do_sim_free(sim).unwrap();
        assert!(
            sims_holding(|s| s.lexicon == "tiny-sim-free")
                .unwrap()
                .is_empty()
        );
        for err in [
            do_sim_step(sim, r#"{"iterations":1}"#, || 0.0).unwrap_err(),
            do_sim_add(sim, &format!("[{PASS}]")).unwrap_err(),
            do_sim_free(sim).unwrap_err(),
        ] {
            assert!(err.contains("freed"), "{err}");
        }
        assert!(evict_kwg("tiny-sim-free".into()).unwrap());
        // a sim of a lexicon no longer cached
        let err = do_sim_new(&format!(
            r#"{{"rack":[{S}],"board":{},"lexicon":"tiny-sim-free","leave":"noleave","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0}}"#,
            cat_board()
        ))
        .unwrap_err();
        assert_eq!(err, "missing kwg");
    }

    // a late board, as json: CAT across row 2 from column 6, and, from row 7
    // on, every other tile but those the racks hold (a blank as an A), which no
    // play can touch, its runs being no words
    fn late_board(racks: &[u8]) -> String {
        late_board_of("CrosswordGame", racks)
    }

    // the same under these rules: their board and their tiles
    fn late_board_of(rules: &str, racks: &[u8]) -> String {
        do_this_on_startup().unwrap();
        let game_config = CACHED_GAME_CONFIG.read().unwrap()[rules].clone();
        let alphabet = game_config.alphabet();
        let mut left = (0..alphabet.len())
            .map(|tile| alphabet.freq(tile))
            .collect::<Vec<_>>();
        for &tile in racks.iter().chain(&[C, A, T]) {
            left[tile as usize] -= 1;
        }
        let dim = game_config.board_layout().dim();
        let mut board = vec![vec![0i8; dim.cols as usize]; dim.rows as usize];
        board[2][6] = C as i8;
        board[2][7] = A as i8;
        board[2][8] = T as i8;
        let mut squares = board[7..].iter_mut().flatten();
        for (tile, &count) in left.iter().enumerate() {
            for _ in 0..count {
                *squares.next().unwrap() = if tile == 0 { -(A as i8) } else { tile as i8 };
            }
        }
        serde_json::to_string(&board).unwrap()
    }

    // an endgame of the late board, seat 0 to move, with what more is given
    fn endgame_of(lexicon: &str, rack: &[u8], others: &[u8], more: &str) -> Result<u32, String> {
        tiny_lexicon(lexicon);
        let racks = rack.iter().chain(others).copied().collect::<Vec<_>>();
        do_endgame_new(&format!(
            r#"{{"rack":{rack:?},"board":{},"lexicon":"{lexicon}","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0{more}}}"#,
            late_board(&racks)
        ))
    }

    // a clock that never moves, so no step runs out of time
    fn frozen() -> std::time::Duration {
        std::time::Duration::ZERO
    }

    fn endgame_step(handle: u32, req: &str) -> serde_json::Value {
        report(&do_endgame_step(handle, req, frozen).unwrap())
    }

    // an empty bag: the other side holds the tiles unseen, and the answer is
    // wolges's own solve of the position, its line played from its first move
    #[test]
    fn an_endgame_answers_as_wolges_solves_it() {
        let (rack, others) = ([S, E, A, T], [A, T, E]);
        let handle = endgame_of("tiny-end-solve", &rack, &others, "").unwrap();
        // read before any step: nothing solved, the shape already whole
        let before = endgame_step(handle, r#"{"ms":0}"#);
        assert_eq!(
            before,
            serde_json::json!({"bag": 0, "steps": 0, "exact": false, "play": null,
                "value": 0.0, "spread": 0.0, "depth": 0, "line": []})
        );
        let answer = endgame_step(handle, r#"{"ms":60000}"#);
        assert_eq!(answer["bag"], 0);
        assert_eq!(answer["steps"], 1);
        assert_eq!(answer["exact"], true);
        assert_eq!(answer["play"], answer["line"][0]["play"]);
        assert_eq!(answer["value"], answer["line"][0]["value"]);
        assert_eq!(answer["spread"], answer["value"]);

        // wolges's solver on the same position
        let Tables {
            kwg, game_config, ..
        } = tables("tiny-end-solve", "noleave", "CrosswordGame").unwrap();
        let mut kibitzer = kibitzer::Kibitzer::new();
        let board = serde_json::from_str::<Vec<Vec<i8>>>(&late_board(
            &rack.iter().chain(&others).copied().collect::<Vec<_>>(),
        ))
        .unwrap();
        kibitzer.prepare(&game_config, &rack, &board).unwrap();
        let arm_tables = anagrams::ArmTables::None;
        let mut solver = endgame::EndgameSolver::<kwg::Node24, kwg::Node24>::new(
            &game_config,
            &kwg.kwg,
            &arm_tables,
        )
        .unwrap();
        let mut sorted = rack.to_vec();
        sorted.sort_unstable();
        solver.init(&kibitzer.board_tiles, [&sorted, &others]);
        let value = solver.solve(0).unwrap();
        assert_eq!(answer["value"].as_f64().unwrap(), points(value));

        // an exact answer stays: a later step solves nothing, and ms 0 reads it
        assert_eq!(endgame_step(handle, r#"{"ms":1000}"#), answer);
        assert_eq!(endgame_step(handle, r#"{"ms":0}"#), answer);
        // an endgame keeps its own graph: one made before its lexicon is
        // evicted solves after it as the first did
        let again = endgame_of("tiny-end-solve", &rack, &others, "").unwrap();
        assert!(evict_kwg("tiny-end-solve".into()).unwrap());
        assert_eq!(endgame_step(again, r#"{"ms":60000}"#), answer);
        do_endgame_free(again).unwrap();
        do_endgame_free(handle).unwrap();
        for err in [
            do_endgame_step(handle, r#"{"ms":1}"#, frozen).unwrap_err(),
            do_endgame_free(handle).unwrap_err(),
        ] {
            assert!(err.contains("freed"), "{err}");
        }
    }

    // the same on the super board and under jumbled rules, on a graph of the
    // words' alphagrams: the answer is wolges's own solve of each
    #[test]
    fn an_endgame_answers_as_wolges_solves_it_on_super_and_jumbled_rules() {
        let (rack, others) = ([S, E, A, T], [A, T, E]);
        let racks = rack.iter().chain(&others).copied().collect::<Vec<_>>();
        for (rules, jumbled) in [("CrosswordGameSuper", false), ("WordSmog", true)] {
            let lexicon = format!("tiny-end-{rules}");
            tiny_lexicon_for(&lexicon, jumbled);
            let board = late_board_of(rules, &racks);
            let handle = do_endgame_new(&format!(
                r#"{{"rack":{rack:?},"board":{board},"lexicon":"{lexicon}","rules":"{rules}","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0}}"#
            ))
            .unwrap();
            let answer = endgame_step(handle, r#"{"ms":60000}"#);
            assert_eq!(answer["exact"], true, "{rules}");
            assert_eq!(answer["play"], answer["line"][0]["play"], "{rules}");

            let Tables {
                kwg, game_config, ..
            } = tables(&lexicon, "noleave", rules).unwrap();
            let mut kibitzer = kibitzer::Kibitzer::new();
            let board = serde_json::from_str::<Vec<Vec<i8>>>(&board).unwrap();
            kibitzer.prepare(&game_config, &rack, &board).unwrap();
            let arm_tables = anagrams::ArmTables::None;
            let mut solver = endgame::EndgameSolver::<kwg::Node24, kwg::Node24>::new(
                &game_config,
                &kwg.kwg,
                &arm_tables,
            )
            .unwrap();
            let mut sorted = rack.to_vec();
            sorted.sort_unstable();
            solver.init(&kibitzer.board_tiles, [&sorted, &others]);
            let value = solver.solve(0).unwrap();
            assert_eq!(answer["value"].as_f64().unwrap(), points(value), "{rules}");
            do_endgame_free(handle).unwrap();
        }
    }

    // one tile in the bag: each tile it may be, weighed by how many of it are
    // unseen; the value is their weighted mean, the win their weighted share
    #[test]
    fn an_endgame_with_a_tile_in_the_bag_weighs_each_tile() {
        let rack = [S, E, A, T, C, A, T];
        let others = [A, T, E, S, E, A, T, E];
        let handle = endgame_of("tiny-end-bag", &rack, &others, "").unwrap();
        // read before any step: no depth searched, nothing to play
        let before = endgame_step(handle, r#"{"ms":0}"#);
        assert_eq!(
            (&before["depth"], &before["play"]),
            (&0.into(), &serde_json::Value::Null)
        );
        let answer = endgame_step(handle, r#"{"ms":60000}"#);
        assert_eq!(answer["bag"], 1);
        assert_eq!(answer["exact"], true);
        assert!(answer["depth"].as_i64().unwrap() >= 1, "{answer}");
        assert!(answer["play"].is_object(), "{answer}");
        let tiles = answer["tiles"].as_array().unwrap();
        let count = |t: &serde_json::Value| t["count"].as_f64().unwrap();
        let value = |t: &serde_json::Value| t["value"].as_f64().unwrap();
        let total = tiles.iter().map(count).sum::<f64>();
        assert_eq!(total, others.len() as f64);
        let mean = tiles.iter().map(|t| count(t) * value(t)).sum::<f64>() / total;
        assert!(
            (answer["value"].as_f64().unwrap() - mean).abs() < 0.001,
            "{answer}"
        );
        let won = (tiles.iter())
            .map(|t| {
                count(t)
                    * match value(t).total_cmp(&0.0) {
                        std::cmp::Ordering::Greater => 1.0,
                        std::cmp::Ordering::Equal => 0.5,
                        std::cmp::Ordering::Less => 0.0,
                    }
            })
            .sum::<f64>()
            / total;
        assert!(
            (answer["win"].as_f64().unwrap() - won).abs() < 1e-6,
            "{answer}"
        );
        do_endgame_free(handle).unwrap();
    }

    // an empty bag's answer gives way to a step's only when that is exact,
    // deeper, or at its depth worth more, so a step that stopped sooner than
    // one before it leaves the deeper answer
    #[test]
    fn an_empty_bags_answer_gives_way_only_to_a_better_one() {
        let answer = |exact: bool, depth: i8, value: f64| EndgameAnswer {
            bag: 0,
            steps: 1,
            exact,
            play: Some(kibitzer::JsonPlay::Exchange {
                tiles: Vec::new().into(),
            }),
            value,
            spread: value,
            depth: Some(depth),
            line: Some(Vec::new()),
            win: None,
            tiles: None,
        };
        let held = answer(false, 3, 5.0);
        assert!(held.gives_way_to(&answer(false, 4, -9.0)), "deeper");
        assert!(held.gives_way_to(&answer(false, 3, 6.0)), "worth more");
        assert!(held.gives_way_to(&answer(true, 2, 1.0)), "exact");
        assert!(!held.gives_way_to(&answer(false, 3, 5.0)), "the same");
        assert!(!held.gives_way_to(&answer(false, 3, 4.0)), "worth less");
        assert!(!held.gives_way_to(&answer(false, 2, 9.0)), "shallower");
        let mut none = answer(false, 0, 0.0);
        none.play = None;
        assert!(none.gives_way_to(&answer(false, 1, -3.0)), "no answer yet");
        // an exact answer from a shallower search than the one held keeps the
        // deeper depth; a step that does not give way counts its step
        let mut held = answer(false, 5, 19.0);
        held.take(answer(true, 3, 19.0), false);
        assert!(held.exact, "the exact answer taken");
        assert_eq!(held.depth, Some(5), "the depth fell");
        let mut steps = answer(false, 5, 19.0);
        steps.take(
            EndgameAnswer {
                steps: 7,
                ..answer(false, 4, 30.0)
            },
            false,
        );
        assert_eq!((steps.depth, steps.value, steps.steps), (Some(5), 19.0, 7));
        // with a tile in the bag every step's answer is taken, its depth never
        // below the one before
        let mut in_bag = answer(false, 2, 1.0);
        in_bag.take(answer(false, 1, 2.0), true);
        assert_eq!((in_bag.depth, in_bag.value), (Some(2), 2.0));
    }

    // what endgame_new and endgame_step refuse, each before it solves
    #[test]
    fn an_endgame_refuses_what_it_cannot_solve() {
        let full = [S, E, A, T, C, A, T];
        for (rack, others, more, wanted) in [
            (
                &full[..],
                &[A, T, E, S, E, A, T, E, E][..],
                "",
                "endgame: the bag holds 2 tiles; at most one is solved",
            ),
            (
                &[S, E, A, T][..],
                &[A, T, E, S, E, A, T, E][..],
                "",
                "endgame: with a tile in the bag the rack must be full",
            ),
            (
                &[S, E][..],
                &[][..],
                "",
                "endgame: the other side holds no tiles, so the game is over",
            ),
            (
                &[S][..],
                &[A, T][..],
                r#","max_memory_mb":0"#,
                "max_memory_mb: 1 to 4096, found 0",
            ),
        ] {
            let err = endgame_of("tiny-end-refuse", rack, others, more).unwrap_err();
            assert_eq!(err, wanted);
        }
        tiny_lexicon("tiny-end-refuse");
        let board = late_board(&[A, T, E]);
        let err = do_endgame_new(&format!(
            r#"{{"rack":[],"board":{board},"lexicon":"tiny-end-refuse","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0}}"#
        ))
        .unwrap_err();
        assert_eq!(err, "endgame: the rack is empty");
        // the seat to move, the scoreless turns, and the passes so far are the
        // position's: left out, refused
        for field in ["turn", "zero_turns", "pass_turns"] {
            let req = format!(
                r#"{{"rack":[{S}],"board":{board},"lexicon":"tiny-end-refuse","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0}}"#
            )
            .replace(&format!(r#","{field}":0"#), "");
            let err = do_endgame_new(&req).unwrap_err();
            assert!(err.contains(&format!("missing field `{field}`")), "{err}");
        }
        let handle = endgame_of("tiny-end-refuse", &[S], &[A, T], "").unwrap();
        assert_eq!(
            do_endgame_step(handle, "{}", frozen).unwrap_err(),
            "endgame_step: give ms"
        );
        assert_eq!(
            do_endgame_step(handle, r#"{"ms":-1}"#, frozen).unwrap_err(),
            "ms: -1 is not a time to run"
        );
        do_endgame_free(handle).unwrap();
    }
}
