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

// a step's prune_z for a sim with no iteration budget, after its iterations so
// far: wolges's move_picker::stitched_z, whose paper is cited there, keeping
// the chance the sim ever retires its best candidate behind a given other one
// to at most alpha however long it runs. Iterations a whole number of at least
// 1 and below 2^64, alpha above 0 and below 1
#[wasm_bindgen]
pub fn stitched_z(iterations: f64, alpha: f64) -> Result<f64, JsValue> {
    Ok(do_stitched_z(iterations, alpha)?)
}

fn do_stitched_z(iterations: f64, alpha: f64) -> Result<f64, String> {
    if !(iterations >= 1.0 && iterations < u64::MAX as f64 && iterations.fract() == 0.0) {
        return Err(format!(
            "iterations: a whole number of at least 1 and below 2^64, found {iterations}"
        ));
    }
    if !(alpha > 0.0 && alpha < 1.0) {
        return Err(format!("alpha: above 0 and below 1, found {alpha}"));
    }
    Ok(move_picker::stitched_z(iterations as u64, alpha))
}

#[wasm_bindgen(start)]
pub fn do_this_on_startup() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    CACHED_KLV.write().unwrap().insert(
        "noleave".into(),
        CachedKlv::new(Klv::empty().map_err(err_to_str)?)?.into(),
    );
    // each game is made once: its WordSmog form is its CrosswordGame one made
    // jumbled (GameConfig::jumbled). So each alphabet and board is built once.
    for (classic, jumbled, make) in [
        (
            "CrosswordGame",
            "WordSmog",
            game_config::make_english_game_config as fn() -> game_config::GameConfig,
        ),
        (
            "CrosswordGameSuper",
            "WordSmogSuper",
            game_config::make_super_english_game_config,
        ),
        (
            "CrosswordGame/catalan",
            "WordSmog/catalan",
            game_config::make_catalan_game_config,
        ),
        (
            "CrosswordGameSuper/catalan",
            "WordSmogSuper/catalan",
            game_config::make_super_catalan_game_config,
        ),
        (
            "CrosswordGame/dutch",
            "WordSmog/dutch",
            game_config::make_dutch_game_config,
        ),
        (
            "CrosswordGame/french",
            "WordSmog/french",
            game_config::make_french_game_config,
        ),
        (
            "CrosswordGame/german",
            "WordSmog/german",
            game_config::make_german_game_config,
        ),
        (
            "CrosswordGameSuper/german",
            "WordSmogSuper/german",
            game_config::make_super_german_game_config,
        ),
        (
            "CrosswordGame/italian",
            "WordSmog/italian",
            game_config::make_italian_game_config,
        ),
        (
            "CrosswordGame/norwegian",
            "WordSmog/norwegian",
            game_config::make_norwegian_game_config,
        ),
        (
            "CrosswordGame/polish",
            "WordSmog/polish",
            game_config::make_polish_game_config,
        ),
        (
            "CrosswordGame/slovene",
            "WordSmog/slovene",
            game_config::make_slovene_game_config,
        ),
        (
            "CrosswordGame/spanish",
            "WordSmog/spanish",
            game_config::make_spanish_game_config,
        ),
        (
            "CrosswordGame/swedish",
            "WordSmog/swedish",
            game_config::make_swedish_game_config,
        ),
    ] {
        let gc = make();
        let jumbled_gc = gc.jumbled();
        let mut cached = CACHED_GAME_CONFIG.write().unwrap();
        cached.insert(classic.into(), gc.into());
        cached.insert(jumbled.into(), jumbled_gc.into());
    }
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
// exchanges: how many exchanges the rack's player has made (0 if not given),
//  so rules with a cap offer none past it.
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
    #[serde(default)]
    exchanges: i16,
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
    Ok(cache_klv(key, table)?)
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
    Ok(cache_klv(key, table)?)
}

// a leave table cached under its name, in place of any before it, and the
// inference tables made from that one forgotten
fn cache_klv(key: String, table: CachedKlv) -> Result<(), String> {
    forget_infer_tables(&key)?;
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
// doing any work, so this drops the name; the memory goes back to the allocator
// when the last user (a live sim among them) is done with it. A later call
// naming an evicted key gets the usual "missing kwg", which is the point -- the
// failure is loud.
#[wasm_bindgen]
pub fn evict_kwg(key: String) -> Result<bool, JsValue> {
    Ok(CACHED_KWG
        .write()
        .map_err(err_to_str)?
        .remove(&key)
        .is_some())
}

// Same, for a leave-value table. See evict_kwg.
#[wasm_bindgen]
pub fn evict_klv(key: String) -> Result<bool, JsValue> {
    forget_infer_tables(&key)?;
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

    if req.exchanges < 0 {
        return_js_error!(format!("exchanges: must be >= 0, found {}", req.exchanges));
    }
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

    let mut move_generator =
        movegen::KurniaMoveGenerator::<u32, false, false>::try_new(&game_config)
            .map_err(err_to_str)?;
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
                num_exchanges_by_this_player: req.exchanges,
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
    // the cross sets wolges's move generator reads: for each square, the letters
    // that make the run of tiles down through it a word and those that make the
    // run across one, each 0 when no tile is beside it that way
    let mut hooks = vec![(0u64, 0u64); kibitzer.board_tiles.len()];
    movegen::KurniaMoveGenerator::<u32, false, false>::try_new(&game_config)
        .map_err(err_to_str)?
        .gen_hooks(
            &movegen::BoardSnapshot {
                board_tiles: &kibitzer.board_tiles,
                game_config: &game_config,
                kwg: &kwg.kwg,
                anagrams: None,
                rack_lengths: None,
                klv: &Klv::empty().map_err(err_to_str)?,
            },
            |idx, down, across| hooks[idx] = (down, across),
        );
    let alphabet_len = game_config.alphabet().len();
    let letters = |bits: u64| {
        (bits != 0).then(|| {
            (1..alphabet_len)
                .filter(|&tile| bits >> tile & 1 != 0)
                .collect::<Vec<_>>()
        })
    };
    let mut squares = Vec::new();
    for r in 0..dim.rows {
        for c in 0..dim.cols {
            let idx = r as usize * dim.cols as usize + c as usize;
            let (down, across) = hooks[idx];
            if kibitzer.board_tiles[idx] != 0 || down | across == 0 {
                continue;
            }
            squares.push(HookSquare {
                row: r,
                col: c,
                across: letters(across),
                down: letters(down),
            });
        }
    }
    serde_json::to_string(&HooksAnswer { squares }).map_err(err_to_str)
}

// infer: the leaves the player who just moved may have kept, from that move, as
// wolges's sims infer them: the board before it, the rack of the one who
// watches (empty for one who sees none), the move as a sim takes one (an
// exchange's tiles read only for how many), how far it may fall short of a
// rack's best (fuzz, in points), and how many of the likeliest leaves to list
// (none unless asked). The leaves, weighted to sum to 1 and millions of them at
// times, stay here under a handle sim_new and sim_set take; none when nothing
// is inferred, as when the bag is empty or the move's draw empties it
#[derive(serde::Deserialize)]
struct InferRequest {
    rack: Vec<u8>,
    #[serde(rename = "board")]
    board_tiles: Vec<Vec<i8>>,
    lexicon: String,
    leave: String,
    rules: String,
    play: SimPlay,
    fuzz: f64,
    #[serde(default)]
    list: usize,
}

// a leave a player may have kept, and its weight
#[derive(Clone, serde::Deserialize, serde::Serialize)]
struct WeightedLeave {
    tiles: Vec<u8>,
    weight: f64,
}

// an inference: its handle, how many leaves it holds, and as many of the
// likeliest as were asked, the likeliest first, those alike in their tiles'
// order
#[derive(serde::Serialize)]
struct InferAnswer {
    handle: u32,
    count: usize,
    leaves: Vec<WeightedLeave>,
    tiles: Vec<InferTile>, // one a tile, in tile order (0 the blank)
}

// a tile (its place in the list), what the inference says of it: the chance the
// opponent's rack holds one or more, the rack its kept leave by the inference's
// weights and the rest dealt from the tiles unseen less that leave (held), or
// dealt from the tiles unseen alone (prior)
#[derive(serde::Serialize)]
struct InferTile {
    held: f64,
    prior: f64,
}

// each tile's InferTile: unseen, the tiles unseen after the play (a count each
// tile); rack, the tiles the opponent's rack holds; leaves, the inference's
// leaves, their weights summing to 1 (none: held is prior)
fn infer_tiles<'a>(
    unseen: &[u8],
    rack: usize,
    leaves: impl Iterator<Item = (&'a [u8], f64)>,
) -> Vec<InferTile> {
    let total = unseen.iter().map(|&count| count as usize).sum::<usize>();
    // the chance a rack of more tiles dealt from left (n tiles) holds tile
    let dealt = |left: &[u8], n: usize, more: usize, tile: usize| {
        let others = n - left[tile] as usize;
        1.0 - choose(others, more) / choose(n, more)
    };
    let prior = (0..unseen.len())
        .map(|tile| dealt(unseen, total, rack.min(total), tile))
        .collect::<Vec<_>>();
    let mut held = vec![0.0; unseen.len()];
    let mut left = vec![0u8; unseen.len()];
    let mut any = false;
    for (leave, weight) in leaves {
        any = true;
        left.copy_from_slice(unseen);
        for &tile in leave {
            left[tile as usize] = left[tile as usize].saturating_sub(1);
        }
        let n = total - leave.len().min(total);
        let more = rack.saturating_sub(leave.len()).min(n);
        for (tile, held) in held.iter_mut().enumerate() {
            *held += weight
                * if leave.contains(&(tile as u8)) {
                    1.0
                } else {
                    dealt(&left, n, more, tile)
                };
        }
    }
    let held = if any { held } else { prior.clone() };
    (held.into_iter().zip(prior))
        .map(|(held, prior)| InferTile { held, prior })
        .collect()
}

// the inferences made, by handle, each its leaves (shared with the sims given
// them); a handle is never used twice
#[derive(Default)]
struct Inferences {
    made: u32,
    live: fash::MyHashMap<u32, std::sync::Arc<infer::PackedLeaves>>,
}

static INFERENCES: std::sync::LazyLock<std::sync::Mutex<Inferences>> =
    std::sync::LazyLock::new(Default::default);

fn no_inference(handle: u32) -> String {
    format!("inference {handle}: none such, or freed")
}

// the n likeliest of these leaves (every one, if no more), as InferAnswer lists
// them: found by a selection, so that a few of millions are not a whole sort
fn likeliest(leaves: &infer::PackedLeaves, n: usize) -> Vec<WeightedLeave> {
    let n = n.min(leaves.len());
    if n == 0 {
        return Vec::new();
    }
    let order = |a: &u32, b: &u32| {
        let (a, b) = (*a as usize, *b as usize);
        (leaves.weight(b).total_cmp(&leaves.weight(a)))
            .then_with(|| leaves.leave(a).cmp(leaves.leave(b)))
    };
    let mut at = (0..leaves.len() as u32).collect::<Vec<_>>();
    if n < at.len() {
        at.select_nth_unstable_by(n - 1, order);
        at.truncate(n);
    }
    at.sort_unstable_by(order);
    (at.into_iter())
        .map(|i| WeightedLeave {
            tiles: leaves.leave(i as usize).to_vec(),
            weight: leaves.weight(i as usize),
        })
        .collect()
}

// the inference tables made for a rules and a leave table (the multiset lattice
// with the table's values on it, and an inference's scratch)
struct HeldInfer {
    rules: String,
    leave: String,
    tables: infer::InferTables,
}

// the inference tables of the rules and the leave table infer was last asked
// about, made by the first infer to need them. One set at a time, as a set
// holds two numbers for each multiset of up to a rack's tiles: infer asked
// about other rules or another table makes its own in their place, and the
// table loaded again or evicted takes them along
static CACHED_INFER: std::sync::Mutex<Option<HeldInfer>> = std::sync::Mutex::new(None);

fn forget_infer_tables(leave: &str) -> Result<(), String> {
    let mut cached = CACHED_INFER.lock().map_err(err_to_str)?;
    if cached.as_ref().is_some_and(|held| held.leave == leave) {
        *cached = None;
    }
    Ok(())
}

// the leaves inferred, as InferAnswer
#[wasm_bindgen]
pub fn infer(req_str: String) -> Result<String, JsValue> {
    Ok(do_infer(&req_str)?)
}

fn do_infer(req_str: &str) -> Result<String, String> {
    let req = serde_json::from_str::<InferRequest>(req_str).map_err(err_to_str)?;
    if let SimPlay::Play {
        bonus,
        challenged_off,
        ..
    } = &req.play
        && (*bonus != 0 || *challenged_off)
    {
        return Err("play: a bonus or a play challenged off says nothing of a leave".into());
    }
    let Tables {
        kwg,
        klv,
        game_config,
    } = tables(&req.lexicon, &req.leave, &req.rules)?;
    let mut kibitzer = kibitzer::Kibitzer::new();
    kibitzer
        .prepare(&game_config, &req.rack, &req.board_tiles)
        .map_err(err_to_str)?;
    let mut move_generator =
        movegen::KurniaMoveGenerator::<u32, false, false>::try_new(&game_config)
            .map_err(err_to_str)?;
    let mut cached = CACHED_INFER.lock().map_err(err_to_str)?;
    let tables = match &mut *cached {
        Some(held) if held.rules == req.rules && held.leave == req.leave => &mut held.tables,
        slot => {
            // the set held goes before the next is made, never two at once
            *slot = None;
            let tables = infer::InferTables::new(&game_config, &klv.klv).map_err(err_to_str)?;
            &mut slot
                .insert(HeldInfer {
                    rules: req.rules.clone(),
                    leave: req.leave.clone(),
                    tables,
                })
                .tables
        }
    };
    let mut leaves = infer::kept_leaves_packed_lean(
        tables,
        &mut move_generator,
        &movegen::BoardSnapshot {
            board_tiles: &kibitzer.board_tiles,
            game_config: &game_config,
            kwg: &kwg.kwg,
            anagrams: None,
            rack_lengths: None,
            klv: &klv.klv,
        },
        &req.rack,
        &req.play.to_play(),
        req.fuzz,
        1,
    )
    .map_err(err_to_str)?;
    drop(cached);
    leaves.normalize();
    let listed = likeliest(&leaves, req.list);
    let count = leaves.len();
    // the tiles unseen after the play: those it placed are on the board
    let mut unseen = kibitzer.available_tally.clone();
    if let movegen::Play::Place { word, .. } = req.play.to_play() {
        for &tile in word.iter().filter(|&&tile| tile != 0) {
            let tile = if tile & 0x80 != 0 { 0 } else { tile as usize };
            unseen[tile] = unseen[tile].saturating_sub(1);
        }
    }
    let num_unseen = unseen.iter().map(|&count| count as usize).sum::<usize>();
    let rack = (game_config.rack_size() as usize).min(num_unseen);
    let tiles = infer_tiles(
        &unseen,
        rack,
        (0..count).map(|i| (leaves.leave(i), leaves.weight(i))),
    );
    let mut inferences = INFERENCES.lock().map_err(err_to_str)?;
    let handle = (inferences.made.checked_add(1)).ok_or("too many inferences made")?;
    inferences.made = handle;
    inferences.live.insert(handle, std::sync::Arc::new(leaves));
    drop(inferences);
    serde_json::to_string(&InferAnswer {
        handle,
        count,
        leaves: listed,
        tiles,
    })
    .map_err(err_to_str)
}

// an inference let go; a sim given it keeps its leaves
#[wasm_bindgen]
pub fn infer_free(handle: u32) -> Result<(), JsValue> {
    Ok(do_infer_free(handle)?)
}

fn do_infer_free(handle: u32) -> Result<(), String> {
    let mut inferences = INFERENCES.lock().map_err(err_to_str)?;
    (inferences.live.remove(&handle))
        .map(|_| ())
        .ok_or_else(|| no_inference(handle))
}

// sim_new: a position (rack, board, lexicon, leave, rules, each seat's score in
// points, the seat to move, and the scoreless turns and the passes in a row so
// far; each seat's rack size, by default the mover's rack with full racks for
// the others dealt in turn from the tiles unseen while enough are left), the
// tiles known to be on the opponent's rack (none by default), the plies each
// rollout plays (always given, null for every ply to the game's end; past them,
// while the bag is empty, it plays on to the game's end), the seed its draws
// come from (a random one if none), and, for a sim run in parts, which part of
// how many this is. A new sim holds no candidates.
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
    #[serde(deserialize_with = "nullable")]
    plies: Option<usize>, // null: every ply to the game's end
    #[serde(default)]
    kept: Vec<WeightedLeave>, // the leaves the opponent may hold, as the caller weighs them
    inferred: Option<u32>, // an inference's leaves the opponent may hold, by infer's handle
    #[serde(default)]
    opponents_pass: bool, // every seat but the mover's passes in a rollout
    seed: Option<u64>,
    #[serde(default)]
    part: u64,
    #[serde(default = "one_part")]
    parts: u64,
}

// a field the request must give, null among the values it may take
fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    <Option<T> as serde::Deserialize>::deserialize(deserializer)
}

fn one_part() -> u64 {
    1
}

// a play as a caller names one to a sim: as play_score takes one, but its score
// may be left out, since the sim counts it again. Other fields are not read, so
// a play as an answer gives it back may be passed as it is. A placement may add
// "bonus" points to its score (a challenge's, as "[and +5]"), or be
// "challenged_off" the board: it scores nothing, the tiles go back on the rack,
// and its turn counts as a scoreless one.
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
        #[serde(default)]
        bonus: i32,
        #[serde(default)]
        challenged_off: bool,
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
                ..
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
// it. Whether to prune, said on every step; to prune, every prune_every
// iterations each running candidate behind the leader by prune_z standard
// errors, on the iterations both ran, is pruned, a chance to win within
// prune_decided (0 to 0.5) of 0 or 1 having its game decided. And a play each
// candidate is compared with, on the iterations both ran.
#[derive(serde::Deserialize)]
struct SimStepRequest {
    iterations: Option<u64>,
    ms: Option<f64>,
    prune: bool,
    prune_every: Option<u64>,
    prune_z: Option<f64>,
    prune_decided: Option<f64>,
    against: Option<SimPlay>,
}

// how a step prunes: every so many of the sim's iterations, behind by so many
// standard errors, decided within so much of 0 or 1
#[derive(Clone, Copy, Debug)]
struct Pruning {
    every: u64,
    z: f64,
    decided: f64,
}

// the pruning a step asks for, each of its settings given and checked; none
// when it asks for none
fn pruning_of(req: &SimStepRequest) -> Result<Option<Pruning>, String> {
    if !req.prune {
        return Ok(None);
    }
    let every = req.prune_every.ok_or("prune_every: required to prune")?;
    if every == 0 {
        return Err("prune_every: must be 1 or more".into());
    }
    let z = req.prune_z.ok_or("prune_z: required to prune")?;
    if z < 0.0 {
        return Err("prune_z: must be 0 or more".into());
    }
    let decided = req
        .prune_decided
        .ok_or("prune_decided: required to prune")?;
    if !(0.0..=0.5).contains(&decided) {
        return Err("prune_decided: must be from 0 to 0.5".into());
    }
    Ok(Some(Pruning { every, z, decided }))
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
// spread with the leaves left counted (the sim's equity, in points) and its two
// parts (the change in spread in points alone, and the leaves left: the mover's
// less the opponent's), the chance to win, and each ply's
struct SimCandidate {
    play: movegen::Play,
    challenged_off: bool,
    static_equity: f32,
    state: SimState,
    first_iter: u64,
    wins: Vec<f64>,
    equities: Vec<f64>,
    objective: stats::Stats,
    equity: stats::Stats,
    spread: stats::Stats,
    leaves: stats::Stats,
    win: stats::Stats,
    plies: Vec<SimPly>,
}

// a sim a caller steps, stops, resumes, and changes: the position, the tables
// it was made with (held here, so that evicting their names cannot pull them
// away), the simmer prepared on it, the plies a rollout plays (none: every ply
// to the game's end), its seed, which part of how many it is, its iterations so
// far, and its candidates
struct Sim {
    game_config: std::sync::Arc<game_config::GameConfig>,
    kwg: std::sync::Arc<CachedKwg>,
    klv: std::sync::Arc<CachedKlv>,
    game_state: game_state::GameState,
    simmer: simmer::Simmer<simmer::DefaultRng, u32, false, false>,
    pristine: game_state::GameState,
    plies: Option<usize>,
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
// it), the change in spread, and the chance to win, then that change in spread
// in its two parts (in points alone, and the leaves left); its plies are the
// simmer's to read until the next
fn rollout(
    simmer: &mut simmer::Simmer<simmer::DefaultRng, u32, false, false>,
    game_config: &game_config::GameConfig,
    kwg: &Kwg,
    klv: &Klv,
    play: &movegen::Play,
    challenged_off: bool,
) -> Result<(f64, i32, f64, i32, i32), String> {
    let no_tables = &anagrams::ArmTables::None;
    let game_ended = if challenged_off {
        simmer.simulate_lean_challenged_off(game_config, kwg, no_tables, klv)
    } else {
        simmer.simulate_lean(game_config, kwg, no_tables, klv, play)
    }
    .map_err(err_to_str)?;
    let final_spread = simmer.final_equity_spread().map_err(err_to_str)?;
    let win = simmer.compute_win_prob(game_ended, final_spread, None);
    let spread = final_spread
        .checked_sub(simmer.initial_score_spread)
        .ok_or("score overflow")?;
    let (points, leaves) = simmer.final_spread_and_leaves().map_err(err_to_str)?;
    let points = points
        .checked_sub(simmer.initial_score_spread)
        .ok_or("score overflow")?;
    Ok((
        simmer::sim_objective(spread, win, simmer.win_prob_weightage()),
        spread,
        win,
        points,
        leaves,
    ))
}

// a play as a sim holds it: checked and scored, its static equity, and whether
// it was challenged off the board, which makes it a candidate of its own
struct HeldPlay {
    play: movegen::Play,
    static_equity: f32,
    challenged_off: bool,
}

impl HeldPlay {
    fn is(&self, candidate: &SimCandidate) -> bool {
        candidate.play == self.play && candidate.challenged_off == self.challenged_off
    }
}

impl Sim {
    // a play as the sim holds it, checked and scored as play_score checks and
    // scores it, with its static equity: refused only if it is no play on this
    // position. A word the lexicon lacks stands, as a phony no one challenged
    // would. Its bonus adds to its score. Challenged off, it scores nothing. The
    // rack then stays whole, so its static equity is a pass's.
    fn held(&self, sim_play: &SimPlay) -> Result<HeldPlay, String> {
        let (bonus, challenged_off) = match sim_play {
            SimPlay::Play {
                bonus,
                challenged_off,
                ..
            } => (*bonus, *challenged_off),
            SimPlay::Exchange { .. } => (0, false),
        };
        if challenged_off && bonus != 0 {
            return Err("a play challenged off scores nothing, so takes no bonus".into());
        }
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
        let score = if challenged_off {
            0
        } else {
            bonus
                .checked_mul(equity::SCALE)
                .and_then(|bonus| ps.compute_score(board_snapshot, &play).checked_add(bonus))
                .ok_or_else(|| format!("bonus: {bonus} points overflow the score"))?
        };
        if let movegen::Play::Place { score: s, .. } = &mut play {
            *s = score;
        }
        let pass = movegen::Play::Exchange {
            tiles: [][..].into(),
        };
        let equity = ps.compute_equity(
            board_snapshot,
            &self.game_state,
            if challenged_off { &pass } else { &play },
            move_filter::LEAVE_SCALE_DENOM,
            score,
        );
        Ok(HeldPlay {
            play,
            static_equity: equity as f32 / equity::SCALE as f32,
            challenged_off,
        })
    }

    // up to max iterations, until out_of_time says so before one, while a
    // candidate runs; each rolls out every running candidate on the same draws,
    // and with pruning, those behind the leader are pruned as it says
    fn step(
        &mut self,
        max: u64,
        pruning: Option<Pruning>,
        mut out_of_time: impl FnMut() -> bool,
    ) -> Result<u64, String> {
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
                let (objective, spread, win, points, leaves) = rollout(
                    &mut self.simmer,
                    &self.game_config,
                    &self.kwg.kwg,
                    &self.klv.klv,
                    &candidate.play,
                    candidate.challenged_off,
                )?;
                let played = self.simmer.plies_played();
                let num_plies = self.plies.map_or(played.len(), |p| played.len().min(p));
                plies.extend_from_slice(&played[..num_plies]);
                rows.push((objective, spread, win, points, leaves, num_plies));
            }
            let mut row = rows.iter();
            let mut at = 0;
            for candidate in &mut self.candidates {
                if candidate.state != SimState::Running {
                    continue;
                }
                let Some(&(objective, spread, win, points, leaves, num_plies)) = row.next() else {
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
                candidate.spread.update(simmer::spread_points(points));
                candidate.leaves.update(simmer::spread_points(leaves));
                candidate.win.update(win);
                // a ply's stats from the first rollout to reach it on
                if candidate.plies.len() < num_plies {
                    candidate.plies.resize_with(num_plies, SimPly::default);
                }
                for (ply, played) in candidate.plies.iter_mut().zip(&plies[at..at + num_plies]) {
                    ply.score.update(simmer::spread_points(played.score));
                    ply.bingo
                        .update(if played.tiles == rack_size { 1.0 } else { 0.0 });
                }
                at += num_plies;
            }
            self.iterations = iteration;
            stepped += 1;
            if let Some(pruning) = pruning
                && iteration.is_multiple_of(pruning.every)
            {
                prune_behind(&mut self.candidates, pruning.z, pruning.decided);
            }
        }
        Ok(stepped)
    }

    fn report(&self, stepped: u64, against: Option<&HeldPlay>) -> SimReport {
        let weight = self.simmer.win_prob_weightage();
        let other = against.and_then(|held| self.candidates.iter().find(|c| held.is(c)));
        let other_objectives = other.map(|other| objectives(other, weight));
        // every ply of the sim's is reported, one no rollout has reached yet as
        // an empty one; a sim to the game's end, every ply a rollout reached
        let unreached = SimPly::default();
        let listed = self.plies.unwrap_or_else(|| {
            (self.candidates.iter())
                .map(|c| c.plies.len())
                .max()
                .unwrap_or(0)
        });
        SimReport {
            iterations: self.iterations,
            stepped,
            candidates: (self.candidates.iter())
                .map(|c| SimCandidateReport {
                    play: kibitzer::JsonPlayWithEquity {
                        equity: c.static_equity,
                        play: (&c.play).into(),
                    },
                    challenged_off: c.challenged_off,
                    state: c.state.name(),
                    n: c.objective.count() as u64,
                    win: (&c.win).into(),
                    equity: (&c.equity).into(),
                    spread: (&c.spread).into(),
                    leaves: (&c.leaves).into(),
                    objective: (&c.objective).into(),
                    plies: (0..listed)
                        .map(|i| c.plies.get(i).unwrap_or(&unreached))
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

// how a chance to win has its game: won, lost, or still open
#[derive(Clone, Copy, PartialEq, Debug)]
enum Outcome {
    Won,
    Lost,
    Open,
}

// a chance to win within decided of 1 has its game won, within it of 0 lost
fn outcome(win: f64, decided: f64) -> Outcome {
    if win >= 1.0 - decided {
        Outcome::Won
    } else if win <= decided {
        Outcome::Lost
    } else {
        Outcome::Open
    }
}

// the running candidate a page sorting by the chance to win lists first: if any
// game is won, the won one of the highest equity; if every game is lost, the
// one of the highest equity; else the one of the highest chance to win
fn leader(candidates: &[SimCandidate], decided: f64) -> Option<&SimCandidate> {
    let running = || (candidates.iter()).filter(|c| c.state == SimState::Running);
    let by_equity =
        |a: &&SimCandidate, b: &&SimCandidate| a.equity.mean().total_cmp(&b.equity.mean());
    let won = |c: &&SimCandidate| outcome(c.win.mean(), decided) == Outcome::Won;
    if running().any(|c| won(&c)) {
        running().filter(won).max_by(by_equity)
    } else if running().all(|c| outcome(c.win.mean(), decided) == Outcome::Lost) {
        running().max_by(by_equity)
    } else {
        running().max_by(|a, b| a.win.mean().total_cmp(&b.win.mean()))
    }
}

// whether a candidate is compared with the leader by equity: when both games
// are won, or both lost, the chance to win cannot tell them apart
fn by_equity(win: f64, lead_win: f64, decided: f64) -> bool {
    let mine = outcome(win, decided);
    mine != Outcome::Open && mine == outcome(lead_win, decided)
}

// the running candidates behind the leader on the iterations both ran, pruned:
// behind by the chance to win, or, where the chance to win cannot tell them
// apart, by equity, each as wolges's move picker tells a candidate behind
fn prune_behind(candidates: &mut [SimCandidate], z: f64, decided: f64) {
    let Some(lead) = leader(candidates, decided) else {
        return;
    };
    let behind = (candidates.iter())
        .map(|c| {
            let (mine, leads) = if by_equity(c.win.mean(), lead.win.mean(), decided) {
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
// the static one), whether it was challenged off (said only when it was), where
// it stands, and its rollouts: their number, chance to win, equity (the change
// in spread in points, the leaves left counted), that equity's two parts
// (spread, the change in spread in points alone; leaves, the mover's leave left
// less the opponent's), objective, and plies (the first the opponent's reply);
// with a play to compare against, itself less that play
#[derive(serde::Serialize)]
struct SimCandidateReport {
    play: kibitzer::JsonPlayWithEquity,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    challenged_off: bool,
    state: &'static str,
    n: u64,
    win: SimStat,
    equity: SimStat,
    spread: SimStat,
    leaves: SimStat,
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
    // the tiles known to be on the opponent's rack, or the leaves it may hold:
    // each iteration puts them there first (a leave chosen by its weight) and
    // deals the rest of it from the tiles unseen
    let opponent = (turn + 1) % num_players;
    let (known, inferred) = match opponent_leaves(
        (req.known, req.kept, req.inferred),
        num_players,
        rack_sizes[opponent] as usize,
        &kibitzer.available_tally,
    )? {
        OpponentLeaves::Listed(known) => (known, None),
        OpponentLeaves::Inferred(leaves) => (Vec::new(), Some(leaves)),
    };

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
    let mut simmer = simmer::Simmer::with_generator(
        &game_config,
        movegen::KurniaMoveGenerator::try_new(&game_config).map_err(err_to_str)?,
    )
    .map_err(err_to_str)?;
    simmer.set_opponents_pass(req.opponents_pass);
    simmer.prepare(
        &game_config,
        &game_state,
        req.plies.unwrap_or(usize::MAX),
        true,
        &known,
    );
    if let Some(leaves) = inferred {
        simmer.set_packed_kept_leaves(opponent as u8, leaves);
    }
    let pristine = simmer.prepared_state().clone();
    let sim = Sim {
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

// tiles known to be on the opponent's rack (a request's field what): a game of
// two seats, no more than that rack holds, all among the tiles unseen (a tally)
fn check_known(
    what: &str,
    known: &[u8],
    num_players: usize,
    opponent_rack: usize,
    unseen: &[u8],
) -> Result<(), String> {
    if num_players != 2 {
        return Err(format!("{what}: only with two seats, found {num_players}"));
    }
    if known.len() > opponent_rack {
        return Err(format!(
            "{what}: {} tiles, but the opponent's rack holds {opponent_rack}",
            known.len(),
        ));
    }
    let mut unseen = unseen.to_vec();
    for &tile in known {
        match unseen.get_mut(tile as usize) {
            Some(count) if *count > 0 => *count -= 1,
            _ => {
                return Err(format!(
                    "{what}: {known:?} are not all among the tiles unseen"
                ));
            }
        }
    }
    Ok(())
}

// the leaves the opponent may hold, weighted, from a request's known tiles (one
// leave, certain) or its kept leaves (as infer answers them), never both; each
// leave checked as known tiles are, each weight 0 or more, and the weights
// summing to a number above 0 (a sim draws a leave from 0 up to their sum)
fn kept_leaves_of(
    known: Vec<u8>,
    kept: Vec<WeightedLeave>,
    num_players: usize,
    opponent_rack: usize,
    unseen: &[u8],
) -> Result<Vec<(Vec<u8>, f64)>, String> {
    if !known.is_empty() {
        if !kept.is_empty() {
            return Err("known, kept: give one or the other".into());
        }
        check_known("known", &known, num_players, opponent_rack, unseen)?;
        return Ok(vec![(known, 1.0)]);
    }
    let mut leaves = Vec::with_capacity(kept.len());
    for (i, leave) in kept.into_iter().enumerate() {
        let what = format!("kept[{i}]");
        if leave.weight < 0.0 {
            return Err(format!("{what}: weight must be 0 or more"));
        }
        check_known(&what, &leave.tiles, num_players, opponent_rack, unseen)?;
        leaves.push((leave.tiles, leave.weight));
    }
    let total = leaves.iter().map(|(_, weight)| weight).sum::<f64>();
    if !leaves.is_empty() && !(total > 0.0 && total.is_finite()) {
        return Err("kept: weights must sum to a finite number above 0".into());
    }
    Ok(leaves)
}

// what the opponent may hold: a list (kept_leaves_of's), or an inference's
// leaves, shared with it
enum OpponentLeaves {
    Listed(Vec<(Vec<u8>, f64)>),
    Inferred(std::sync::Arc<infer::PackedLeaves>),
}

// what the opponent may hold from a request's known tiles, its kept leaves, or
// an inference (by infer's handle), never two of them; an inference's leaves
// checked as known tiles are, none of them copied
fn opponent_leaves(
    (known, kept, inferred): (Vec<u8>, Vec<WeightedLeave>, Option<u32>),
    num_players: usize,
    opponent_rack: usize,
    unseen: &[u8],
) -> Result<OpponentLeaves, String> {
    let Some(handle) = inferred else {
        return Ok(OpponentLeaves::Listed(kept_leaves_of(
            known,
            kept,
            num_players,
            opponent_rack,
            unseen,
        )?));
    };
    if !known.is_empty() {
        return Err("known, inferred: give one or the other".into());
    }
    if !kept.is_empty() {
        return Err("kept, inferred: give one or the other".into());
    }
    let leaves = (INFERENCES.lock().map_err(err_to_str)?.live.get(&handle))
        .cloned()
        .ok_or_else(|| format!("inferred: {}", no_inference(handle)))?;
    if num_players != 2 && !leaves.is_empty() {
        return Err(format!(
            "inferred: only with two seats, found {num_players}"
        ));
    }
    let mut left = vec![0u8; unseen.len()];
    for i in 0..leaves.len() {
        let leave = leaves.leave(i);
        if leave.len() > opponent_rack {
            return Err(format!(
                "inferred: leave {i} has {} tiles, but the opponent's rack holds {opponent_rack}",
                leave.len()
            ));
        }
        left.copy_from_slice(unseen);
        for &tile in leave {
            match left.get_mut(tile as usize) {
                Some(count) if *count > 0 => *count -= 1,
                _ => {
                    return Err(format!(
                        "inferred: leave {i}, {leave:?}, is not all among the tiles unseen"
                    ));
                }
            }
        }
    }
    Ok(OpponentLeaves::Inferred(leaves))
}

// threats: what the opponent may play next, by score alone (no leave is read).
// A board, lexicon and rules as analyze takes them; the tiles unseen as unseen
// (a count each tile) or as the rack of the one who watches, not both;
// opp_rack_size (a rack's tiles, or all unseen when fewer); what the opponent
// holds as sim_new takes it (one of known, kept (or leaves) and inferred; none:
// a rack dealt from the tiles unseen); count, how many best plays to list (all
// if not given); ms, a time to run, listing included (none: no limit;
// threats_new ignores it); seed, for the racks drawn when ms runs out; square,
// [row, col], to list only the plays placing a tile there. The tiles the
// watcher would draw stay unseen, so a board with a play on it asks after it.
#[derive(serde::Deserialize)]
struct ThreatsRequest {
    #[serde(default)]
    rack: Vec<u8>,
    #[serde(rename = "board")]
    board_tiles: Vec<Vec<i8>>,
    lexicon: String,
    rules: String,
    unseen: Option<Vec<u8>>,
    opp_rack_size: Option<usize>,
    #[serde(default)]
    known: Vec<u8>,
    #[serde(default, alias = "leaves")]
    kept: Vec<WeightedLeave>,
    inferred: Option<u32>,
    count: Option<usize>,
    ms: Option<f64>,
    seed: Option<u64>,
    square: Option<[usize; 2]>,
}

// a play the opponent may make as analyze gives one (its score in points).
// tiles: the tiles it places in tile order (0 for a blank). chance: the chance
// the opponent's rack holds them.
#[derive(serde::Serialize)]
struct ThreatPlay {
    #[serde(flatten)]
    play: kibitzer::JsonPlay,
    tiles: Vec<u8>,
    chance: f64,
}

// an empty square. best: the best score in points of a play placing a tile on
// it (0 if none does). chance: the chance the opponent holds a play that does
// and scores that much. expected: the expected score in points of the
// opponent's best play placing a tile on it (a rack with none scores 0).
// reach: the chance the opponent holds any play placing a tile on it.
#[derive(serde::Serialize)]
struct ThreatSquare {
    best: i32,
    chance: f64,
    expected: f64,
    reach: f64,
}

// what the opponent may play, by its rack's chance of holding the tiles. exact:
// false when ms ran out before every rack was counted, every chance and
// expected score then the likeliest racks' exact sums plus an unbiased estimate
// of the rest's from racks drawn among them. plays: the best first (those alike
// in score in placement order).
#[derive(serde::Serialize)]
struct ThreatsAnswer {
    racks: usize,       // how many distinct racks it may hold
    count: usize,       // how many plays it may make of the tiles unseen
    expected_best: f64, // its best play's expected score (0 with no play)
    bingo: Option<f64>, // a whole rack's play's chance (null: a short rack)
    covered: f64,       // the chance the racks counted exactly hold
    samples: usize,     // how many racks were drawn
    list_ms: f64,       // listing every play's time, which ms cannot cut
    exact: bool,
    plays: Vec<ThreatPlay>,
    squares: Vec<Vec<Option<ThreatSquare>>>, // row by row; null where a tile is
}

// the ways to choose k of n, as a float
fn choose(n: usize, k: usize) -> f64 {
    (0..k).fold(1.0, |ways, i| ways * (n - i) as f64 / (i + 1) as f64)
}

// each way to deal more tiles from tally (a count each tile) from tile on,
// found with the ways to deal it: held, the tiles dealt so far, in tile order,
// and rest, the tiles in tally from each tile on
fn deal(
    tally: &[u8],
    rest: &[usize],
    tile: usize,
    more: usize,
    ways: f64,
    held: &mut Vec<u8>,
    found: &mut dyn FnMut(&[u8], f64),
) {
    if more == 0 {
        found(held, ways);
        return;
    }
    if rest[tile] < more {
        return;
    }
    let len = held.len();
    for take in 0..=more.min(tally[tile] as usize) {
        if take > 0 {
            held.push(tile as u8);
        }
        let ways = ways * choose(tally[tile] as usize, take);
        deal(tally, rest, tile + 1, more - take, ways, held, found);
    }
    held.truncate(len);
}

// a play's squares it places a tile on, each as row * cols + col
fn placed_squares(
    (down, lane, idx, word): (bool, i8, i8, &[u8]),
    cols: usize,
) -> impl Iterator<Item = usize> + '_ {
    (idx..)
        .zip(word)
        .filter(|&(_, &tile)| tile != 0)
        .map(move |(i, _)| {
            let (row, col) = if down { (i, lane) } else { (lane, i) };
            row as usize * cols + col as usize
        })
}

// a play the opponent may make, its tiles' multiset by its id, its word at word
// in the buffer of every play's words
struct ThreatFound {
    down: bool,
    lane: i8,
    idx: i8,
    word: std::ops::Range<u32>,
    score: i32,
    id: u32,
}

// the sums over racks of what the opponent may hold: each multiset's chance,
// each square's chance, expected best and reach, the best play's expected score
// and the chance of a bingo
struct ThreatSums {
    holds: Vec<f64>,
    square_chance: Vec<f64>,
    square_expected: Vec<f64>,
    square_reach: Vec<f64>,
    expected_best: f64,
    bingo: f64,
}

impl ThreatSums {
    fn new(multisets: usize, squares: usize) -> Self {
        Self {
            holds: vec![0.0; multisets],
            square_chance: vec![0.0; squares],
            square_expected: vec![0.0; squares],
            square_reach: vec![0.0; squares],
            expected_best: 0.0,
            bingo: 0.0,
        }
    }

    // these sums plus each times those
    fn plus(mut self, those: &Self, each: f64) -> Self {
        let add = |sums: &mut [f64], more: &[f64]| {
            for (sum, more) in sums.iter_mut().zip(more) {
                *sum += each * more;
            }
        };
        add(&mut self.holds, &those.holds);
        add(&mut self.square_chance, &those.square_chance);
        add(&mut self.square_expected, &those.square_expected);
        add(&mut self.square_reach, &those.square_reach);
        self.expected_best += each * those.expected_best;
        self.bingo += each * those.bingo;
        self
    }
}

// what weighing a rack reads: each multiset's id and best score, each
// multiset's squares with its best score through each, and each square's best
struct ThreatTables<'a> {
    ids: &'a fash::MyHashMap<Box<[u8]>, u32>,
    best: &'a [i32],
    reach: &'a [(u32, usize, i32)],
    reach_from: &'a [usize],
    square_best: &'a [i32],
    rack_size: usize,
}

// a rack's weighing, its scratch kept from rack to rack: each square's best
// score through it for the rack stamped
struct ThreatWeigher {
    rack_square: Vec<(usize, i32)>,
    touched: Vec<usize>,
    distinct: Vec<(u8, u8)>,
    taken: Vec<u8>,
    sub: Vec<u8>,
    stamp: usize,
}

impl ThreatWeigher {
    // the rack (its tiles in tile order) weighed by weight into sums
    fn weigh(
        &mut self,
        tables: &ThreatTables<'_>,
        rack: &[u8],
        weight: f64,
        sums: &mut ThreatSums,
    ) {
        self.stamp += 1;
        let stamp = self.stamp;
        self.distinct.clear();
        for &tile in rack {
            match self.distinct.last_mut() {
                Some((last, count)) if *last == tile => *count += 1,
                _ => self.distinct.push((tile, 1)),
            }
        }
        self.taken.clear();
        self.taken.resize(self.distinct.len(), 0);
        self.touched.clear();
        let mut top = 0;
        let mut whole = false;
        while let Some(j) = (0..self.distinct.len()).find(|&j| self.taken[j] < self.distinct[j].1) {
            self.taken[..j].fill(0);
            self.taken[j] += 1;
            self.sub.clear();
            for (&(tile, _), &n) in self.distinct.iter().zip(&self.taken) {
                self.sub.extend(std::iter::repeat_n(tile, n as usize));
            }
            let Some(&id) = tables.ids.get(&self.sub[..]) else {
                continue;
            };
            let id = id as usize;
            sums.holds[id] += weight;
            top = top.max(tables.best[id]);
            whole |= self.sub.len() == tables.rack_size;
            let reach = &tables.reach[tables.reach_from[id]..tables.reach_from[id + 1]];
            for &(_, at, score) in reach {
                let (seen, square) = &mut self.rack_square[at];
                if *seen != stamp {
                    *seen = stamp;
                    *square = score;
                    self.touched.push(at);
                } else {
                    *square = (*square).max(score);
                }
            }
        }
        for &at in &self.touched {
            let score = self.rack_square[at].1;
            sums.square_expected[at] += weight * score as f64;
            sums.square_reach[at] += weight;
            if score == tables.square_best[at] {
                sums.square_chance[at] += weight;
            }
        }
        sums.expected_best += weight * top as f64;
        if whole {
            sums.bingo += weight;
        }
    }
}

// a threats question made, every play listed and every rack dealt, weighed a
// step at a time: the racks counted exactly so far (the likeliest first), their
// sums and what an answer reads
struct Threats {
    ids: fash::MyHashMap<Box<[u8]>, u32>,
    keys: Vec<Box<[u8]>>,
    best: Vec<i32>,
    plays: Vec<ThreatFound>, // every play, the best first
    words: Vec<u8>,          // every play's word, each at its play's word
    count: usize,            // how many plays there are
    count_asked: Option<usize>,
    reach: Vec<(u32, usize, i32)>,
    reach_from: Vec<usize>,
    square_best: Vec<i32>,
    board_tiles: Vec<u8>, // each square's tile, 0 for none
    cols: usize,
    rack_size: usize,
    opponent_rack: usize,
    racks: Vec<(Box<[u8]>, f64)>,
    counted: usize,
    covered: f64,
    exact_sums: ThreatSums,
    weigher: ThreatWeigher,
    rng: rand::rngs::ChaCha20Rng,
    list_ms: f64,
    listed: f64,                     // when listing every play ended
    drawn: (ThreatSums, f64, usize), // the last weighing's draws, each's weight, their number
}

// the threats made, by handle; a handle is never used twice
#[derive(Default)]
struct ThreatsMade {
    made: u32,
    live: fash::MyHashMap<u32, Threats>,
}

static THREATS: std::sync::LazyLock<std::sync::Mutex<ThreatsMade>> =
    std::sync::LazyLock::new(Default::default);

fn no_threats(handle: u32) -> String {
    format!("threats {handle}: none such, or freed")
}

// what threats is asked, every play listed (ms not read)
fn threats_made(req: ThreatsRequest, now: &impl Fn() -> f64) -> Result<Threats, String> {
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
    let rack_size = game_config.rack_size() as usize;
    if req.rack.len() > rack_size {
        return Err(format!(
            "rack: max {rack_size} tiles, found {} tiles",
            req.rack.len()
        ));
    }
    let mut kibitzer = kibitzer::Kibitzer::new();
    kibitzer
        .prepare(&game_config, &req.rack, &req.board_tiles)
        .map_err(err_to_str)?;
    if let Some(unseen) = req.unseen {
        if !req.rack.is_empty() {
            return Err("rack, unseen: give one or the other".into());
        }
        let alphabet_len = game_config.alphabet().len() as usize;
        if unseen.len() != alphabet_len {
            return Err(format!(
                "unseen: a count each of {alphabet_len} tiles, found {} counts",
                unseen.len()
            ));
        }
        kibitzer.available_tally = unseen;
    }
    let unseen = &kibitzer.available_tally;
    let num_unseen = unseen.iter().map(|&count| count as usize).sum::<usize>();
    let opponent_rack = match req.opp_rack_size {
        None => rack_size.min(num_unseen),
        Some(size) if size <= rack_size && size <= num_unseen => size,
        Some(size) => {
            return Err(format!(
                "opp_rack_size: max {} tiles, found {size} tiles",
                rack_size.min(num_unseen)
            ));
        }
    };
    let opponent = opponent_leaves(
        (req.known, req.kept, req.inferred),
        game_config.num_players() as usize,
        opponent_rack,
        unseen,
    )?;
    let leaves = match &opponent {
        OpponentLeaves::Listed(listed) if listed.is_empty() => vec![(&[][..], 1.0)],
        OpponentLeaves::Listed(listed) => (listed.iter())
            .map(|(tiles, weight)| (&tiles[..], *weight))
            .collect(),
        OpponentLeaves::Inferred(packed) => (0..packed.len())
            .map(|i| (packed.leave(i), packed.weight(i)))
            .collect::<Vec<_>>(),
    };
    let total = leaves.iter().map(|&(_, weight)| weight).sum::<f64>();

    // the racks the opponent may hold, each its chance: a leave's chance, and
    // the rest of the rack dealt from the tiles unseen but the leave's
    let mut racks = fash::MyHashMap::<Box<[u8]>, f64>::default();
    let mut left = vec![0u8; unseen.len()];
    let mut rest = vec![0usize; unseen.len() + 1];
    let mut held = Vec::new();
    let mut rack = Vec::new();
    for &(leave, weight) in &leaves {
        if weight == 0.0 {
            continue;
        }
        left.copy_from_slice(unseen);
        for &tile in leave {
            left[tile as usize] -= 1;
        }
        for tile in (0..left.len()).rev() {
            rest[tile] = rest[tile + 1] + left[tile] as usize;
        }
        let more = opponent_rack - leave.len();
        let chance = weight / total / choose(rest[0], more);
        deal(
            &left,
            &rest,
            0,
            more,
            chance,
            &mut held,
            &mut |dealt, ways| {
                rack.clear();
                rack.extend_from_slice(leave);
                rack.extend_from_slice(dealt);
                rack.sort_unstable();
                *racks.entry(rack[..].into()).or_insert(0.0) += ways;
            },
        );
    }
    // the likeliest racks first, so a time cut weighs the most of them
    let mut racks = racks.into_iter().collect::<Vec<_>>();
    racks.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    // every play of the tiles unseen, its tiles' multiset given an id as it is
    // first found, with each multiset's best score
    let mut move_generator =
        movegen::KurniaMoveGenerator::<u32, false, false>::try_new(&game_config)
            .map_err(err_to_str)?;
    let empty_klv = Klv::empty().map_err(err_to_str)?;
    let board_snapshot = movegen::BoardSnapshot {
        board_tiles: &kibitzer.board_tiles,
        game_config: &game_config,
        kwg: &kwg.kwg,
        anagrams: None,
        rack_lengths: None,
        klv: &empty_klv,
    };
    let pool = (0u8..)
        .zip(unseen.iter())
        .flat_map(|(tile, &count)| std::iter::repeat_n(tile, count as usize))
        .collect::<Vec<_>>();
    let listing = now();
    let mut ids = fash::MyHashMap::<Box<[u8]>, u32>::default();
    let mut keys = Vec::<Box<[u8]>>::new();
    let mut best = Vec::<i32>::new();
    let mut plays = Vec::<ThreatFound>::new();
    let mut words = Vec::<u8>::new();
    if !pool.is_empty() {
        let mut key = Vec::new();
        move_generator.pool_plays(&board_snapshot, &pool, |down, lane, idx, word, score| {
            key.clear();
            key.extend(
                word.iter().filter(|&&tile| tile != 0).map(
                    |&tile| {
                        if tile & 0x80 != 0 { 0 } else { tile }
                    },
                ),
            );
            key.sort_unstable();
            let id = match ids.get(&key[..]) {
                Some(&id) => {
                    best[id as usize] = best[id as usize].max(score);
                    id
                }
                None => {
                    let id = keys.len() as u32;
                    ids.insert(key[..].into(), id);
                    keys.push(key[..].into());
                    best.push(score);
                    id
                }
            };
            let from = words.len() as u32;
            words.extend_from_slice(word);
            plays.push(ThreatFound {
                down,
                lane,
                idx,
                word: from..words.len() as u32,
                score,
                id,
            });
        });
    }
    let listed = now();
    let list_ms = listed - listing;

    // each square's best score of a play placing a tile on it, and for each
    // multiset each square a play of it places a tile on, with the best score
    // of such a play
    let dim = game_config.board_layout().dim();
    let cols = dim.cols as usize;
    let mut square_best = vec![i32::MIN; kibitzer.board_tiles.len()];
    let mut reach = Vec::<(u32, usize, i32)>::new();
    let word = |play: &ThreatFound| &words[play.word.start as usize..play.word.end as usize];
    for play in &plays {
        for at in placed_squares((play.down, play.lane, play.idx, word(play)), cols) {
            square_best[at] = square_best[at].max(play.score);
            reach.push((play.id, at, play.score));
        }
    }
    reach.sort_unstable_by(|a, b| (a.0, a.1, b.2).cmp(&(b.0, b.1, a.2)));
    reach.dedup_by_key(|&mut (id, at, _)| (id, at));
    let mut reach_from = vec![0usize; keys.len() + 1];
    for &(id, _, _) in &reach {
        reach_from[id as usize + 1] += 1;
    }
    for id in 0..keys.len() {
        reach_from[id + 1] += reach_from[id];
    }

    let count = plays.len();
    plays.sort_unstable_by(|a, b| {
        (b.score.cmp(&a.score))
            .then_with(|| (a.down, a.lane, a.idx, word(a)).cmp(&(b.down, b.lane, b.idx, word(b))))
    });
    let seed = req
        .seed
        .unwrap_or_else(|| RNG.with(|rng| rng.borrow_mut().next_u64()));
    Ok(Threats {
        exact_sums: ThreatSums::new(keys.len(), square_best.len()),
        weigher: ThreatWeigher {
            rack_square: vec![(usize::MAX, 0i32); square_best.len()],
            touched: Vec::new(),
            distinct: Vec::new(),
            taken: Vec::new(),
            sub: Vec::new(),
            stamp: 0,
        },
        ids,
        keys,
        best,
        plays,
        words,
        count,
        count_asked: req.count,
        reach,
        reach_from,
        square_best,
        board_tiles: kibitzer.board_tiles,
        cols,
        rack_size,
        opponent_rack,
        racks,
        counted: 0,
        covered: 0.0,
        rng: rand::rngs::ChaCha20Rng::seed_from_u64(seed),
        list_ms,
        listed,
        drawn: (ThreatSums::new(0, 0), 0.0, 0), // none yet
    })
}

impl Threats {
    // the racks not yet counted counted exactly, the likeliest first, till half
    // (if given) passes, then racks drawn from those left (each as likely as
    // its chance) till deadline does; the answer, as ThreatsAnswer. Each rack
    // weighs its chance in (ThreatWeigher::weigh), each draw the chance left
    // over the draws. A step that does not weigh answers from the last one's
    // draws. The plays: those placing a tile on square if given, count of them
    // (else as many as asked of the handle).
    fn step(
        &mut self,
        half: Option<f64>,
        deadline: Option<f64>,
        weigh: bool,
        (count, square): (Option<usize>, Option<[usize; 2]>),
        now: &impl Fn() -> f64,
    ) -> Result<String, String> {
        let rows = self.board_tiles.len() / self.cols;
        let through = match square {
            None => None,
            Some([row, col]) if row < rows && col < self.cols => Some(row * self.cols + col),
            Some([row, col]) => return Err(format!("square: [{row}, {col}] is off the board")),
        };
        let tables = ThreatTables {
            ids: &self.ids,
            best: &self.best,
            reach: &self.reach,
            reach_from: &self.reach_from,
            square_best: &self.square_best,
            rack_size: self.rack_size,
        };
        let from = self.counted;
        while weigh && self.counted < self.racks.len() {
            let r = self.counted - from;
            if r.is_multiple_of(16) && r > 0 && half.is_some_and(|half| now() >= half) {
                break;
            }
            let (rack, chance) = &self.racks[self.counted];
            self.weigher
                .weigh(&tables, rack, *chance, &mut self.exact_sums);
            self.covered += *chance;
            self.counted += 1;
        }
        let exact = self.counted == self.racks.len();
        let mut drawn_sums = ThreatSums::new(self.keys.len(), self.square_best.len());
        let mut samples = 0usize;
        let mut rest_chance = 0.0;
        if weigh && !exact {
            let upto = (self.racks[self.counted..].iter())
                .map(|(_, chance)| {
                    rest_chance += chance;
                    rest_chance
                })
                .collect::<Vec<_>>();
            while rest_chance > 0.0
                && !(samples.is_multiple_of(16)
                    && samples > 0
                    && deadline.is_some_and(|deadline| now() >= deadline))
            {
                let at = self.rng.random::<f64>() * rest_chance;
                let i = (upto.partition_point(|&upto| upto <= at)).min(upto.len() - 1);
                let rack = &self.racks[self.counted + i].0;
                self.weigher.weigh(&tables, rack, 1.0, &mut drawn_sums);
                samples += 1;
            }
        }
        let each = if samples > 0 {
            rest_chance / samples as f64
        } else {
            0.0
        };
        if weigh {
            self.drawn = (drawn_sums, each, samples);
        }
        let (drawn_sums, each, samples) = &self.drawn;
        let samples = if exact { 0 } else { *samples };
        let sums = (ThreatSums::new(self.keys.len(), self.square_best.len()))
            .plus(&self.exact_sums, 1.0)
            .plus(drawn_sums, *each);
        let word =
            |play: &ThreatFound| &self.words[play.word.start as usize..play.word.end as usize];
        let plays = (self.plays.iter())
            .filter(|play| {
                through.is_none_or(|through| {
                    placed_squares((play.down, play.lane, play.idx, word(play)), self.cols)
                        .any(|at| at == through)
                })
            })
            .take(count.or(self.count_asked).unwrap_or(usize::MAX))
            .map(|play| ThreatPlay {
                tiles: self.keys[play.id as usize].to_vec(),
                chance: sums.holds[play.id as usize],
                play: (&movegen::Play::Place {
                    down: play.down,
                    lane: play.lane,
                    idx: play.idx,
                    word: word(play).into(),
                    score: play.score,
                })
                    .into(),
            })
            .collect();
        let squares = (self.board_tiles.chunks(self.cols))
            .zip((0..).step_by(self.cols))
            .map(|(row, from)| {
                (row.iter().zip(from..))
                    .map(|(&tile, at)| {
                        (tile == 0).then(|| ThreatSquare {
                            best: equity::descale_score(self.square_best[at].max(0)),
                            chance: sums.square_chance[at],
                            expected: sums.square_expected[at] / equity::SCALE as f64,
                            reach: sums.square_reach[at],
                        })
                    })
                    .collect()
            })
            .collect();
        serde_json::to_string(&ThreatsAnswer {
            racks: self.racks.len(),
            count: self.count,
            expected_best: sums.expected_best / equity::SCALE as f64,
            bingo: (self.opponent_rack == self.rack_size).then_some(sums.bingo),
            covered: if exact { 1.0 } else { self.covered },
            samples,
            list_ms: self.list_ms,
            exact,
            plays,
            squares,
        })
        .map_err(err_to_str)
    }
}

// a time to run, as a request's ms gives it: a deadline from now
fn threats_deadline(ms: Option<f64>, now: &impl Fn() -> f64) -> Result<Option<f64>, String> {
    match ms {
        Some(ms) if ms >= 0.0 && ms.is_finite() => Ok(Some(now() + ms)),
        Some(ms) => Err(format!("ms: {ms} is not a time to run")),
        None => Ok(None),
    }
}

// threats in one call: listing every play counts against ms, the racks counted
// exactly in half of what is left and drawn in the rest
#[wasm_bindgen]
pub fn threats(req_str: String) -> Result<String, JsValue> {
    Ok(do_threats(&req_str, step_clock)?)
}

fn do_threats(req_str: &str, now: impl Fn() -> f64) -> Result<String, String> {
    let req = serde_json::from_str::<ThreatsRequest>(req_str).map_err(err_to_str)?;
    let deadline = threats_deadline(req.ms, &now)?;
    let square = req.square;
    let mut threats = threats_made(req, &now)?;
    let listed = threats.listed;
    let half = deadline.map(|deadline| listed + (deadline - listed).max(0.0) / 2.0);
    threats.step(half, deadline, true, (None, square), &now)
}

// threats_new: a threats question as threats takes it (ms not read), every play
// listed now, its racks left to threats_step; its handle
#[wasm_bindgen]
pub fn threats_new(req_str: String) -> Result<u32, JsValue> {
    Ok(do_threats_new(&req_str, step_clock)?)
}

fn do_threats_new(req_str: &str, now: impl Fn() -> f64) -> Result<u32, String> {
    let req = serde_json::from_str::<ThreatsRequest>(req_str).map_err(err_to_str)?;
    let threats = threats_made(req, &now)?;
    let mut made = THREATS.lock().map_err(err_to_str)?;
    let handle = made.made.checked_add(1).ok_or("too many threats made")?;
    made.made = handle;
    made.live.insert(handle, threats);
    Ok(handle)
}

// threats_step: ms, a time to run (none: every rack counted; 0: nothing more
// weighed, the answer from what was). The racks not yet counted counted exactly
// in the first half of it, then racks drawn from those left in the rest; the
// answer as threats gives it, exact once every rack is counted, its plays those
// through square ([row, col]) if given, count of them if given (else as many as
// threats_new was asked for)
#[derive(serde::Deserialize)]
struct ThreatsStepRequest {
    ms: Option<f64>,
    count: Option<usize>,
    square: Option<[usize; 2]>,
}

#[wasm_bindgen]
pub fn threats_step(threats: u32, req_str: String) -> Result<String, JsValue> {
    Ok(do_threats_step(threats, &req_str, step_clock)?)
}

fn do_threats_step(handle: u32, req_str: &str, now: impl Fn() -> f64) -> Result<String, String> {
    let req = serde_json::from_str::<ThreatsStepRequest>(req_str).map_err(err_to_str)?;
    let deadline = threats_deadline(req.ms, &now)?;
    let half = (req.ms.zip(deadline)).map(|(ms, deadline)| deadline - ms / 2.0);
    let weigh = req.ms != Some(0.0);
    let mut made = THREATS.lock().map_err(err_to_str)?;
    let threats = (made.live.get_mut(&handle)).ok_or_else(|| no_threats(handle))?;
    threats.step(half, deadline, weigh, (req.count, req.square), &now)
}

// a threats question let go, with the plays and racks it holds
#[wasm_bindgen]
pub fn threats_free(threats: u32) -> Result<(), JsValue> {
    Ok(do_threats_free(threats)?)
}

fn do_threats_free(handle: u32) -> Result<(), String> {
    let mut made = THREATS.lock().map_err(err_to_str)?;
    made.live
        .remove(&handle)
        .map(|_| ())
        .ok_or_else(|| no_threats(handle))
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
    let pruning = pruning_of(&req)?;
    let mut sims = SIMS.lock().map_err(err_to_str)?;
    let sim = sims.live.get_mut(&handle).ok_or_else(|| no_sim(handle))?;
    // the play to compare against, checked before any step
    let against = match &req.against {
        Some(sim_play) => {
            let held = sim.held(sim_play).map_err(|e| format!("against: {e}"))?;
            if !sim.candidates.iter().any(|c| held.is(c)) {
                return Err("against: not in the sim".into());
            }
            Some(held)
        }
        None => None,
    };
    let stepped = sim.step(req.iterations.unwrap_or(u64::MAX), pruning, || {
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

// a play a change was given: as the sim holds it, as analyze lists a play,
// whether it was challenged off (said only when it was), and where it stood
// before the change ("absent" if not in the sim)
#[derive(serde::Serialize)]
struct SimChanged {
    play: kibitzer::JsonPlayWithEquity,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    challenged_off: bool,
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
    for held in held {
        let at = sim.candidates.iter().position(|c| held.is(c));
        let was = at.map_or("absent", |at| sim.candidates[at].state.name());
        match (change, at) {
            (SimChange::Add, None) => sim.candidates.push(SimCandidate {
                play: held.play.clone(),
                challenged_off: held.challenged_off,
                static_equity: held.static_equity,
                state: SimState::Running,
                first_iter: sim.iterations + 1,
                wins: Vec::new(),
                equities: Vec::new(),
                objective: stats::Stats::new(),
                equity: stats::Stats::new(),
                spread: stats::Stats::new(),
                leaves: stats::Stats::new(),
                win: stats::Stats::new(),
                plies: Vec::new(),
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
                equity: held.static_equity,
                play: (&held.play).into(),
            },
            challenged_off: held.challenged_off,
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

// sim_set: a change to a live sim's plies or to what the opponent holds, each
// as sim_new takes it (one left out stays as it was); the results so far stay,
// and the change counts from the next iteration on
#[derive(serde::Deserialize)]
struct SimSetRequest {
    #[serde(default, deserialize_with = "given")]
    plies: Option<Option<usize>>, // null: every ply to the game's end
    known: Option<Vec<u8>>,
    kept: Option<Vec<WeightedLeave>>,
    inferred: Option<u32>,
}

// a field given, null among the values it may take; one left out takes its
// default
fn given<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

// the sim after its change, as SimReport
#[wasm_bindgen]
pub fn sim_set(sim: u32, req_str: String) -> Result<String, JsValue> {
    Ok(do_sim_set(sim, &req_str)?)
}

fn do_sim_set(handle: u32, req_str: &str) -> Result<String, String> {
    let req = serde_json::from_str::<SimSetRequest>(req_str).map_err(err_to_str)?;
    let mut sims = SIMS.lock().map_err(err_to_str)?;
    let sim = sims.live.get_mut(&handle).ok_or_else(|| no_sim(handle))?;
    let num_players = sim.game_config.num_players() as usize;
    let turn = sim.game_state.turn as usize;
    let opponent = (turn + 1) % num_players;
    // checked before anything changes: the tiles unseen are the bag's and every
    // rack's but the mover's
    let held = if req.known.is_some() || req.kept.is_some() || req.inferred.is_some() {
        let mut unseen = vec![0u8; sim.game_config.alphabet().len() as usize];
        let others = (sim.game_state.players.iter().enumerate())
            .filter(|&(i, _)| i != turn)
            .flat_map(|(_, player)| player.rack.iter());
        for &tile in sim.game_state.bag.as_slice().iter().chain(others) {
            unseen[tile as usize] += 1;
        }
        Some(opponent_leaves(
            (
                req.known.unwrap_or_default(),
                req.kept.unwrap_or_default(),
                req.inferred,
            ),
            num_players,
            sim.game_state.players[opponent].rack.len(),
            &unseen,
        )?)
    } else {
        None
    };
    if let Some(plies) = req.plies {
        sim.simmer
            .set_num_sim_plies(&sim.game_config, plies.unwrap_or(usize::MAX));
        sim.plies = plies;
    }
    match held {
        Some(OpponentLeaves::Listed(known)) => sim.simmer.set_kept_leaves(opponent as u8, &known),
        Some(OpponentLeaves::Inferred(leaves)) => {
            sim.simmer.set_packed_kept_leaves(opponent as u8, leaves)
        }
        None => {}
    }
    serde_json::to_string(&sim.report(0, None)).map_err(err_to_str)
}

fn do_sim_free(handle: u32) -> Result<(), String> {
    let mut sims = SIMS.lock().map_err(err_to_str)?;
    sims.live
        .remove(&handle)
        .map(|_| ())
        .ok_or_else(|| no_sim(handle))
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

// the bytes the wasm's allocator holds now
#[wasm_bindgen]
pub fn memory_held() -> f64 {
    held_bytes() as f64
}

// the bytes of the wasm's whole linear memory, as an endgame's memory cap
// counts them
#[cfg(target_arch = "wasm32")]
fn page_memory() -> usize {
    core::arch::wasm32::memory_size(0) * 65536
}

#[cfg(not(target_arch = "wasm32"))]
fn page_memory() -> usize {
    held_bytes()
}

// the page's clock as an endgame reads it: performance.now, as a sim's step's
fn page_clock() -> std::time::Duration {
    std::time::Duration::from_secs_f64(step_clock().max(0.0) / 1000.0)
}

// endgame_new: a position (rack, board, lexicon, rules, each seat's score in
// points, the seat to move, and the scoreless turns and the passes in a row so
// far), and the most memory the wasm may hold while it solves, in MiB. With
// none given, the solver's table grows until the page refuses it more; a caller
// whose page may be refused sooner than it can cope with (a phone's browser)
// gives one. With a tile in the bag the rack is full; with two or more, the
// model and the exchanges below are the position's too
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
    #[serde(default)]
    max_memory_mb: Option<usize>,
    // the solver's table from the start, in MiB (none or 0: it grows from one
    // bucket), held to half of max_memory_mb, as a growing table holds its old
    // buckets beside the new ones
    #[serde(default)]
    reserve_mb: Option<usize>,
    // with two tiles or more in the bag: what each player knows once the first
    // move is made ("info_sets" or "random_draws", wolges's PegModel), and the
    // exchanges each seat has made
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    exchanges: Option<[i16; 2]>,
    // an endgame whose graph and table this one takes (an empty bag's what-if)
    #[serde(default)]
    warm_from: Option<u32>,
    // with two tiles or more in the bag, part k of n: the search values only
    // the splits whose index is k mod n, so n endgames (a page's workers), one
    // a part, value every split between them once; each candidate's draws of
    // all n, weighed by their counts, are the whole search's
    #[serde(default)]
    part: Option<[usize; 2]>,
    // with two tiles or more in the bag, whether the search's early passes
    // value every move fast by its equity where a line still leaves tiles in
    // the bag (each such value an estimate, the answer then not exact), later
    // passes refining them to exact; off unless asked
    #[serde(default)]
    estimates: bool,
}

// endgame_step: at most this many milliseconds; 0 reads the last answer. With
// an empty bag, first_moves_ms asks for every first move's value too, on the
// table the steps kept, in at most that many milliseconds more. With two tiles
// or more in the bag, draws_of asks for one candidate's draws (its index in the
// answer's candidates). brief asks for the answer's state alone: no candidates,
// draws, tiles or line
#[derive(serde::Deserialize)]
struct EndgameStepRequest {
    ms: Option<f64>,
    #[serde(default)]
    first_moves_ms: Option<f64>,
    #[serde(default)]
    draws_of: Option<usize>,
    // with draws_of, its rows from the first given to before the second
    #[serde(default)]
    draws_range: Option<[usize; 2]>,
    #[serde(default)]
    brief: bool,
}

// endgame_step's answer when brief: its fields of the state, as they are
#[derive(serde::Serialize)]
struct EndgameBrief<'a> {
    bag: usize,
    steps: u64,
    nodes: u64,
    exact: bool,
    play: &'a Option<kibitzer::JsonPlay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<f64>,
    spread: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    win: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    endgames: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    moves: Option<[usize; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part: Option<[usize; 2]>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    out_of_memory: bool,
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

// tiles the bag may hold, with two or more in it: the deals that give them, the
// play's gain if they are the bag's, in points, and its chance to win then;
// with the model of random draws, the tiles the play then draws, each draw with
// the same (drawn: its deals within the set, its gain, its chance)
#[derive(serde::Serialize)]
struct EndgameBagDraw {
    tiles: Vec<u8>,
    count: u64,
    value: f64,
    win: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    drawn: Vec<EndgameBagDraw>,
}

// a play the search valued: the share of the deals it wins (a tie counted half;
// with an empty bag, 1, a half, or 0 by the lead it ends with) and its mean
// gain, in points; with one tile in the bag, its gain for each tile the bag may
// be; with two or more, for each set it may hold; and whether its value is
// exact
#[derive(serde::Serialize)]
struct EndgameCandidate {
    play: kibitzer::JsonPlay,
    win: f64,
    value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    tiles: Option<Vec<EndgameBagTile>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    draws: Option<Vec<EndgameBagDraw>>,
    // with two or more in the bag, the deals its draws count
    #[serde(skip_serializing_if = "Option::is_none")]
    deals: Option<u64>,
    // with draws_of: the rows its whole draws hold (draws_range pages them)
    #[serde(skip_serializing_if = "Option::is_none")]
    draws_total: Option<usize>,
    exact: bool,
}

// a move's splits with two or more in the bag, as the page reads them, each
// with the draws the move makes there where the model has them
fn bag_draws(
    hypotheses: &[(Vec<u8>, u64, equity::Equity)],
    wins: &[f64],
    draws: &[Vec<endgame::PegBagDraw>],
    lead: f64,
) -> Vec<EndgameBagDraw> {
    (hypotheses.iter().zip(wins).zip(draws))
        .map(|(((tiles, count, margin), &win), draws)| EndgameBagDraw {
            tiles: tiles.clone(),
            count: *count,
            value: points(*margin) - lead,
            win,
            drawn: (draws.iter())
                .map(|draw| EndgameBagDraw {
                    tiles: draw.tiles.clone(),
                    count: draw.ways,
                    value: points(draw.margin) - lead,
                    win: draw.win,
                    drawn: Vec::new(),
                })
                .collect(),
        })
        .collect()
}

// endgame_step's answer: the bag's tiles, the steps that solved and the nodes
// they searched, whether the answer is the endgame's own (the solve finished),
// the play to make and its gain for the side to move to the game's end (none
// before a step solved), and the lead it then ends with, in points; with an
// empty bag the depth searched and the line; with tiles in it the share of the
// deals the play wins, the endgames searched, the moves its pass has valued of
// all it values, and the gain for each tile the bag may be (one tile) or for
// each set it may hold, with every play valued, best first (two or more)
#[derive(serde::Serialize)]
struct EndgameAnswer {
    bag: usize,
    steps: u64,
    nodes: u64,
    exact: bool,
    play: Option<kibitzer::JsonPlay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<f64>,
    spread: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<Vec<EndgamePly>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    win: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tiles: Option<Vec<EndgameBagTile>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    draws: Option<Vec<EndgameBagDraw>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    candidates: Option<Vec<EndgameCandidate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    endgames: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    moves: Option<[usize; 2]>,
    // the part of the search it is, k of n (endgame_new's part)
    #[serde(skip_serializing_if = "Option::is_none")]
    part: Option<[usize; 2]>,
    // the step stopped short of exact holding max_memory_mb or more: the search
    // has no room to go on (left out when not so)
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    out_of_memory: bool,
}

// an endgame a caller steps and frees: its rules and its own word graph, of the
// lexicon's words its board can still take, as wolges's endgame command prunes
// its graph (each held here at one address, for the solver's table to be given
// back to, so the lexicon's name may be evicted while it lives), the position
// (the board, the side to move's rack, the tiles it cannot see with each one's
// count, how many are in the bag, the seat to move, its lead in millipoints,
// and the passes and scoreless turns before it), the memory cap, the solver's
// table between steps, with tiles in the bag the search so far (one tile's, or
// two or more's), and the last answer
struct Endgame {
    game_config: std::sync::Arc<game_config::GameConfig>,
    kwg: std::sync::Arc<Kwg>,
    arm_tables: std::sync::Arc<anagrams::ArmTables>,
    lexicon: String, // the lexicon its graph was pruned from
    rules: String,   // and the rules it was asked of
    board_tiles: Vec<u8>,
    rack: Vec<u8>,
    unseen: Vec<u8>,
    in_bag: bool,
    bag: usize,
    peg_bag: Option<endgame::PegBagProgress>,
    bag_moves: Vec<endgame::PegBagMove>, // the last answer's candidates, two or more in the bag
    turn: u8,
    lead: i32,
    pass_turns: u16,
    zero_turns: u16,
    max_memory: Option<usize>,
    reserve: usize, // the table's bytes a first step reserves, 0 for none
    table: Option<endgame::KeptTable>,
    peg: Option<endgame::PegProgress>,
    finished: Option<(i8, equity::Equity)>, // an empty bag's deepest pass done, its value
    nodes: u64,                             // the nodes an empty bag's steps searched
    answer: EndgameAnswer,
}

impl EndgameAnswer {
    // whether an empty bag's answer gives way to a later step's: the step's is
    // exact, or deeper, or at its depth worth more (a play the step proved
    // better); a step deepens on from the depth the steps before it finished,
    // and one stopped in its first pass answers from that depth
    fn gives_way_to(&self, step: &EndgameAnswer) -> bool {
        self.play.is_none()
            || step.exact
            || step.depth > self.depth
            || (step.depth == self.depth && step.value > self.value)
    }

    // a later step's answer taken when this one gives way to it, always with a
    // tile in the bag (its search resumes, so its answer only deepens); the
    // depth never falls, as an exact answer may come from a shallower search
    // than one before it, the table holding what that one finished. Otherwise
    // only its counts of steps and nodes are taken
    fn take(&mut self, step: EndgameAnswer, in_bag: bool) {
        if in_bag || self.gives_way_to(&step) {
            let depth = step.depth.max(self.depth);
            *self = EndgameAnswer { depth, ..step };
        } else {
            self.steps = step.steps;
            self.nodes = step.nodes;
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
    fn solve(
        &mut self,
        ms: f64,
        clock: endgame::Clock,
        memory: fn() -> usize,
    ) -> Result<(), String> {
        let mut solver =
            endgame::EndgameSolver::<kwg::Node24, kwg::Node24, true, u32, false, false>::with_generator(
                &self.game_config,
                &self.kwg,
                &self.arm_tables,
                movegen::KurniaMoveGenerator::try_new(&self.game_config).map_err(err_to_str)?,
            )
            .map_err(err_to_str)?;
        match self.table.take() {
            Some(table) => {
                if solver.give_table(table).is_err() {
                    return Err("endgame: its table belongs to other tables".into());
                }
            }
            None if self.reserve != 0 => {
                solver.reserve_table(self.reserve);
            }
            None => {}
        }
        solver.set_clock(clock);
        let limits = endgame::Limits {
            time: Some(std::time::Duration::from_secs_f64(ms / 1000.0)),
            states: None,
            memory: self.max_memory.map(|bytes| endgame::MemoryLimit {
                bytes,
                held: memory,
            }),
        };
        let lead = points(equity::Equity::new(self.lead));
        let steps = self.answer.steps + 1;
        let answer = if let Some(progress) = self.peg_bag.as_mut() {
            self.bag_moves = Vec::new();
            solver.set_scoreless_turns(self.pass_turns, self.zero_turns);
            let peg = solver.resume_peg(progress, &limits);
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
            // the search's margins are the leads the game ends with; none yet
            // when a limit stopped it before it valued a move, as before a step
            let valued = peg.best_move.is_some();
            let value = valued.then(|| points(peg.expected_margin) - lead);
            let (endgames, nodes) = progress.searched();
            let (k, n) = progress.part_of();
            let answer = EndgameAnswer {
                part: (n > 1).then_some([k, n]),
                out_of_memory: false,
                bag: self.bag,
                steps,
                nodes,
                exact: peg.exact,
                play: peg.best_move.as_ref().map(Into::into),
                value,
                spread: lead + value.unwrap_or(0.0),
                depth: Some(peg.depth),
                line: None,
                win: valued.then_some(peg.win_pct),
                tiles: None,
                draws: Some(bag_draws(&peg.hypotheses, &peg.wins, &peg.draws, lead)),
                candidates: Some(
                    (peg.moves.iter())
                        .map(|valued| EndgameCandidate {
                            play: (&valued.play).into(),
                            win: valued.win_pct,
                            value: points(valued.expected_margin) - lead,
                            tiles: None,
                            draws: None,
                            deals: Some(valued.hypotheses.iter().map(|h| h.1).sum()),
                            draws_total: None,
                            exact: valued.exact,
                        })
                        .collect(),
                ),
                endgames: Some(endgames),
                moves: progress.moves().map(|(valued, of)| [valued, of]),
            };
            self.bag_moves = peg.moves;
            answer
        } else if let Some(progress) = self.peg.as_mut() {
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
            // the search's margins are the leads the game ends with; none yet
            // when a limit stopped it before a pass valued every tile, as
            // before a step
            let valued = !peg.hypotheses.is_empty();
            let value = valued.then(|| points(peg.expected_margin) - lead);
            let (endgames, nodes) = progress.searched();
            EndgameAnswer {
                part: None,
                out_of_memory: false,
                bag: 1,
                steps,
                nodes,
                exact: peg.exact,
                play: peg.best_move.as_ref().map(peg_play),
                value,
                spread: lead + value.unwrap_or(0.0),
                depth: Some(peg.depth),
                line: None,
                win: valued.then_some(peg.win_pct as f64),
                tiles: Some(
                    (peg.hypotheses.iter())
                        .map(|&(tile, count, margin)| EndgameBagTile {
                            tile,
                            count,
                            value: points(margin) - lead,
                        })
                        .collect(),
                ),
                draws: None,
                candidates: Some(
                    (peg.moves.iter())
                        .map(|valued| EndgameCandidate {
                            play: peg_play(&valued.play),
                            win: valued.win_pct as f64,
                            value: points(valued.expected_margin) - lead,
                            tiles: Some(
                                (valued.hypotheses.iter())
                                    .map(|&(tile, count, margin)| EndgameBagTile {
                                        tile,
                                        count,
                                        value: points(margin) - lead,
                                    })
                                    .collect(),
                            ),
                            draws: None,
                            deals: None,
                            draws_total: None,
                            exact: valued.exact,
                        })
                        .collect(),
                ),
                endgames: Some(endgames),
                moves: progress.moves().map(|(valued, of)| [valued, of]),
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
            let solved = match self.finished {
                Some(finished) => solver.solve_within_after(self.turn, &limits, finished),
                None => solver.solve_within(self.turn, &limits),
            };
            let solved = match solved {
                Ok(solved) => solved,
                Err(e) => {
                    self.table = Some(solver.take_table());
                    return Err(err_to_str(e));
                }
            };
            self.nodes += solver.searched();
            self.finished = Some((solved.depth, solved.value));
            let mut line = Vec::new();
            solver.collect_pv(self.turn, &mut line);
            let value = points(solved.value);
            EndgameAnswer {
                part: None,
                out_of_memory: false,
                bag: 0,
                steps,
                nodes: self.nodes,
                exact: solved.exact,
                play: line.first().map(|(_, play)| play.into()),
                value: Some(value),
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
                draws: None,
                candidates: None,
                endgames: None,
                moves: None,
            }
        };
        self.table = Some(solver.take_table());
        self.answer.take(answer, self.in_bag);
        Ok(())
    }

    // with an empty bag, every first move's value, on the table the steps kept,
    // in at most ms milliseconds, into the answer's candidates: best first,
    // each exact once no line is cut short; one a limit stopped before its
    // first depth is left out
    fn value_first_moves(
        &mut self,
        ms: f64,
        clock: endgame::Clock,
        memory: fn() -> usize,
    ) -> Result<(), String> {
        let mut solver =
            endgame::EndgameSolver::<kwg::Node24, kwg::Node24, true, u32, false, false>::with_generator(
                &self.game_config,
                &self.kwg,
                &self.arm_tables,
                movegen::KurniaMoveGenerator::try_new(&self.game_config).map_err(err_to_str)?,
            )
            .map_err(err_to_str)?;
        match self.table.take() {
            Some(table) => {
                if solver.give_table(table).is_err() {
                    return Err("endgame: its table belongs to other tables".into());
                }
            }
            None if self.reserve != 0 => {
                solver.reserve_table(self.reserve);
            }
            None => {}
        }
        solver.set_clock(clock);
        let limits = endgame::Limits {
            time: Some(std::time::Duration::from_secs_f64(ms / 1000.0)),
            states: None,
            memory: self.max_memory.map(|bytes| endgame::MemoryLimit {
                bytes,
                held: memory,
            }),
        };
        let lead = points(equity::Equity::new(self.lead));
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
        let valued = solver.value_first_moves(self.turn, &limits);
        self.nodes += solver.searched();
        self.table = Some(solver.take_table());
        let valued = valued.map_err(err_to_str)?;
        self.answer.candidates = Some(
            (valued.iter())
                .filter_map(|first| {
                    let value = points(first.value?);
                    let spread = lead + value;
                    Some(EndgameCandidate {
                        play: (&first.play).into(),
                        win: if spread > 0.0 {
                            1.0
                        } else if spread < 0.0 {
                            0.0
                        } else {
                            0.5
                        },
                        value,
                        tiles: None,
                        draws: None,
                        deals: None,
                        draws_total: None,
                        exact: first.exact,
                    })
                })
                .collect(),
        );
        Ok(())
    }
}

// a move of a search with a tile in the bag, as the page reads it (a pass as an
// exchange of no tiles)
fn peg_play(play: &endgame::PegMove) -> kibitzer::JsonPlay {
    match play {
        endgame::PegMove::Pass => kibitzer::JsonPlay::Exchange {
            tiles: Vec::new().into(),
        },
        endgame::PegMove::Place(play) => play.into(),
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
    // a cap past what the page can have is never reached; none at all leaves no
    // room for the table
    if req.max_memory_mb == Some(0) {
        return Err("max_memory_mb: at least 1, found 0".into());
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
    let bag = num_unseen.saturating_sub(rack_size);
    let in_bag = bag != 0;
    if in_bag && req.rack.len() != rack_size {
        return Err("endgame: with a tile in the bag the rack must be full".into());
    }
    // with two tiles or more in the bag, what each player knows and the
    // exchanges made are the position's too
    let bag_search = if bag >= 2 {
        let model = match req.model.as_deref() {
            Some("info_sets") => endgame::PegModel::InfoSets,
            Some("random_draws") => endgame::PegModel::RandomDraws,
            found => {
                return Err(format!(
                    "model: \"info_sets\" or \"random_draws\" with {bag} tiles in the bag, found {found:?}"
                ));
            }
        };
        let exchanges = (req.exchanges)
            .ok_or_else(|| format!("exchanges: each seat's, with {bag} tiles in the bag"))?;
        if exchanges.iter().any(|&made| made < 0) {
            return Err(format!("exchanges: each must be >= 0, found {exchanges:?}"));
        }
        Some((model, exchanges))
    } else {
        None
    };
    let mut rack = req.rack;
    rack.sort_unstable();
    // a what-if warmed by an endgame before it (warm_from): with the bag empty
    // in both, of the same rules and lexicon, on a board holding every tile of
    // that one's on its square, the new one takes that one's graph (its words
    // are all this board can still take) and its table, which then leaves that
    // one, whose later steps start cold
    let warm = match req.warm_from {
        None => None,
        Some(from) => {
            let mut endgames = ENDGAMES.lock().map_err(err_to_str)?;
            let source = endgames
                .live
                .get_mut(&from)
                .ok_or_else(|| format!("warm_from: {}", no_endgame(from)))?;
            if in_bag || source.in_bag {
                return Err("warm_from: both endgames must have an empty bag".into());
            }
            if source.rules != req.rules || source.lexicon != req.lexicon {
                return Err(format!(
                    "warm_from: endgame {from} has other rules or another lexicon"
                ));
            }
            if (source.board_tiles.iter())
                .zip(&kibitzer.board_tiles)
                .any(|(&was, &now)| was != 0 && was != now)
            {
                return Err(format!(
                    "warm_from: the board lacks a tile of endgame {from}'s board"
                ));
            }
            // the source's own game_config too: a table goes only to a solver
            // of the very tables it was made with
            Some((
                std::sync::Arc::clone(&source.game_config),
                std::sync::Arc::clone(&source.kwg),
                std::sync::Arc::clone(&source.arm_tables),
                source
                    .table
                    .take()
                    .map(endgame::KeptTable::for_another_root),
            ))
        }
    };
    let (game_config, kwg, arm_tables, table) = match warm {
        Some(warm) => warm,
        None => {
            // the lexicon's words the board can still take, whatever the racks, made
            // into the endgame's own graph, as wolges's endgame command prunes its own:
            // the same answers, a far smaller graph to generate on
            let mut words = fash::MyHashSet::<bites::Bites>::default();
            movegen::KurniaMoveGenerator::<u32, false, false>::try_new(&game_config)
                .map_err(err_to_str)?
                .gen_remaining_words(
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
            (
                std::sync::Arc::clone(&game_config),
                std::sync::Arc::new(Kwg::from_bytes_alloc(&bytes).map_err(err_to_str)?),
                std::sync::Arc::new(anagrams::ArmTables::None),
                None,
            )
        }
    };
    let peg_bag = match bag_search {
        Some((model, exchanges)) => {
            let mut progress = endgame::PegBagProgress::new(endgame::PegBagPosition {
                mover: req.turn,
                board: kibitzer.board_tiles.clone(),
                mover_rack: rack.clone(),
                unseen_tally: unseen.clone(),
                bag_size: bag,
                score_diff: equity::Equity::new(scores),
                exchanges,
                model,
            })
            .map_err(err_to_str)?;
            if let Some([k, n]) = req.part {
                progress.part(k, n).map_err(err_to_str)?;
            }
            progress.set_estimates(req.estimates);
            Some(progress)
        }
        None if req.estimates => {
            return Err(format!(
                "estimates: with two tiles or more in the bag, found {bag} in it"
            ));
        }
        None if req.part.is_some() => {
            return Err(format!(
                "part: with two tiles or more in the bag, found {bag} in it"
            ));
        }
        None => None,
    };
    let peg = (bag == 1).then(|| {
        endgame::PegProgress::new(
            req.turn,
            &kibitzer.board_tiles,
            &rack,
            &unseen,
            equity::Equity::new(scores),
        )
    });
    // the cap in bytes, as many as a usize holds at most (on wasm32, 4096 MiB
    // shifted would wrap to none)
    let max_memory = req.max_memory_mb.map(|mb| mb.saturating_mul(1 << 20));
    let reserve = (req.reserve_mb.unwrap_or(0).saturating_mul(1 << 20))
        .min(max_memory.map_or(usize::MAX, |bytes| bytes / 2));
    let endgame = Endgame {
        game_config,
        kwg,
        arm_tables,
        lexicon: req.lexicon,
        rules: req.rules,
        board_tiles: kibitzer.board_tiles.clone(),
        rack,
        unseen,
        in_bag,
        bag,
        peg_bag,
        bag_moves: Vec::new(),
        turn: req.turn,
        lead: scores,
        pass_turns: req.pass_turns,
        zero_turns: req.zero_turns,
        max_memory,
        reserve,
        table,
        peg,
        finished: None,
        nodes: 0,
        answer: EndgameAnswer {
            part: req.part.filter(|&[_, n]| n > 1),
            out_of_memory: false,
            bag,
            steps: 0,
            nodes: 0,
            exact: false,
            play: None,
            value: None,
            spread: points(equity::Equity::new(scores)),
            depth: Some(0),
            line: (!in_bag).then(Vec::new),
            win: None,
            tiles: (bag == 1).then(Vec::new),
            draws: (bag >= 2).then(Vec::new),
            candidates: in_bag.then(Vec::new),
            endgames: in_bag.then_some(0),
            moves: None,
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
    Ok(do_endgame_step(handle, &req_str, page_clock, page_memory)?)
}

fn do_endgame_step(
    handle: u32,
    req_str: &str,
    clock: endgame::Clock,
    memory: fn() -> usize,
) -> Result<String, String> {
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
        endgame.solve(ms, clock, memory)?;
    }
    match req.first_moves_ms {
        None => {}
        Some(ms) if !(ms >= 0.0 && ms.is_finite()) => {
            return Err(format!("first_moves_ms: {ms} is not a time to run"));
        }
        Some(_) if endgame.bag != 0 => {
            return Err(
                "first_moves_ms: with tiles in the bag every move is valued in candidates".into(),
            );
        }
        Some(ms) => endgame.value_first_moves(ms, clock, memory)?,
    }
    endgame.answer.out_of_memory =
        !endgame.answer.exact && endgame.max_memory.is_some_and(|bytes| memory() >= bytes);
    if req.brief {
        if req.draws_of.is_some() {
            return Err("brief: no draws come with it, so not with draws_of".into());
        }
        let a = &endgame.answer;
        let brief = EndgameBrief {
            bag: a.bag,
            steps: a.steps,
            nodes: a.nodes,
            exact: a.exact,
            play: &a.play,
            value: a.value,
            spread: a.spread,
            depth: a.depth,
            win: a.win,
            endgames: a.endgames,
            moves: a.moves,
            part: a.part,
            out_of_memory: a.out_of_memory,
        };
        return serde_json::to_string(&brief).map_err(err_to_str);
    }
    let Some(i) = req.draws_of else {
        if req.draws_range.is_some() {
            return Err("draws_range: it pages draws_of's rows, so give draws_of".into());
        }
        return serde_json::to_string(&endgame.answer).map_err(err_to_str);
    };
    let Some(valued) = endgame.bag_moves.get(i) else {
        return Err(format!(
            "draws_of: {i} is no candidate of the {} with draws",
            endgame.bag_moves.len()
        ));
    };
    let lead = points(equity::Equity::new(endgame.lead));
    let total = valued.hypotheses.len();
    let [from, to] = req
        .draws_range
        .unwrap_or([0, total])
        .map(|at| at.min(total));
    let to = to.max(from);
    let draws = bag_draws(
        &valued.hypotheses[from..to],
        &valued.wins[from..to],
        &valued.draws[from..to],
        lead,
    );
    let candidate = &mut endgame.answer.candidates.as_mut().unwrap()[i];
    candidate.draws = Some(draws);
    candidate.draws_total = Some(total);
    let answer = serde_json::to_string(&endgame.answer).map_err(err_to_str);
    let candidate = &mut endgame.answer.candidates.as_mut().unwrap()[i];
    candidate.draws = None;
    candidate.draws_total = None;
    answer
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

    // analyze takes the exchanges the rack's player has made, none if not
    // given, as wolges's i16 (a count past it is refused as the request is
    // read; one below none, by do_analyze)
    #[test]
    fn analyze_takes_the_players_exchanges() {
        let base = r#""rack":[1],"board":[],"count":1,"lexicon":"x","leave":"y","rules":"z""#;
        let given =
            serde_json::from_str::<AnalyzeRequest>(&format!("{{{base},\"exchanges\":3}}")).unwrap();
        assert_eq!(given.exchanges, 3);
        let none = serde_json::from_str::<AnalyzeRequest>(&format!("{{{base}}}")).unwrap();
        assert_eq!(none.exchanges, 0);
        assert!(
            serde_json::from_str::<AnalyzeRequest>(&format!("{{{base},\"exchanges\":32768}}"))
                .is_err()
        );
    }

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

    // every rules name is cached at startup in pairs: each WordSmog one is the
    // CrosswordGame one of its name under the jumbled rules
    #[test]
    fn every_rules_name_is_cached_classic_and_jumbled_in_pairs() {
        do_this_on_startup().unwrap();
        let rules = CACHED_GAME_CONFIG.read().unwrap();
        let mut classic = 0;
        for (name, game_config) in rules.iter() {
            let Some(rest) = name.strip_prefix("CrosswordGame") else {
                assert!(name.starts_with("WordSmog"), "{name}");
                continue;
            };
            classic += 1;
            let jumbled = &rules[&format!("WordSmog{rest}")];
            assert!(
                matches!(game_config.game_rules(), game_config::GameRules::Classic),
                "{name}"
            );
            assert!(
                matches!(jumbled.game_rules(), game_config::GameRules::Jumbled),
                "{name}"
            );
            assert_eq!(jumbled.alphabet().len(), game_config.alphabet().len());
            assert_eq!(
                jumbled.board_layout().dim().rows,
                game_config.board_layout().dim().rows
            );
            assert_eq!(jumbled.rack_size(), game_config.rack_size());
        }
        assert_eq!((classic, rules.len()), (14, 28));
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

    // the same under jumbled rules, on a graph of the words' alphagrams: a
    // run's tiles and the letter make a word in any order, so above A, S as
    // well as T (as), and below T, E as well as A (et)
    #[test]
    fn hooks_under_jumbled_rules_take_the_letters_in_any_order() {
        tiny_lexicon_for("tiny-hooks-jumbled", true);
        let req = format!(
            r#"{{"board":{},"lexicon":"tiny-hooks-jumbled","rules":"WordSmog"}}"#,
            cat_board()
        );
        let answer = serde_json::from_str::<serde_json::Value>(&do_hooks(&req).unwrap()).unwrap();
        assert_eq!(
            answer,
            serde_json::json!({"squares": [
                {"row": 6, "col": 6, "across": null, "down": []},
                {"row": 6, "col": 7, "across": null, "down": [S, T]},
                {"row": 6, "col": 8, "across": null, "down": [A, E]},
                {"row": 7, "col": 5, "across": [S], "down": null},
                {"row": 7, "col": 9, "across": [S], "down": null},
                {"row": 8, "col": 6, "across": null, "down": []},
                {"row": 8, "col": 7, "across": null, "down": [S, T]},
                {"row": 8, "col": 8, "across": null, "down": [A, E]},
            ]})
        );
    }

    // a sim of this board, seed 7, scores level, with what more is given; its
    // plies wolges's own sims' unless that gives them
    fn sim_of(lexicon: &str, board: &str, more: &str) -> u32 {
        tiny_lexicon(lexicon);
        let plies = if more.contains(r#""plies""#) {
            String::new()
        } else {
            format!(r#","plies":{}"#, move_picker::DEFAULT_NUM_SIM_PLIES)
        };
        do_sim_new(&format!(
            r#"{{"rack":[{S},{E},{A},{T}],"board":{board},"lexicon":"{lexicon}","leave":"noleave","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0,"seed":7{plies}{more}}}"#
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

    // a step's fields to prune as wolges's own sims prune (every PRUNE_CADENCE
    // iterations, the z of a whole default sim), decided within this margin
    fn pruning(decided: f64) -> String {
        format!(
            r#""prune":true,"prune_every":{},"prune_z":{:?},"prune_decided":{decided:?}"#,
            move_picker::PRUNE_CADENCE,
            move_picker::paired_z(move_picker::DEFAULT_NUM_SIM_ITERS)
        )
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
        let read = step(b, r#"{"iterations":0,"prune":false}"#);
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
            .map(|&sim| step(sim, r#"{"iterations":4,"prune":false}"#))
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

    // a sim's plies are the caller's: sixteen is no cap, nor is any game's
    // length; each rollout plays on to the game's end once the bag is empty.
    // Every ply is reported, those no rollout reached as empty ones; no later
    // ply counts more rollouts than the one before
    #[test]
    fn a_sim_takes_any_plies_and_reports_each() {
        for plies in [2usize, 20, 1000] {
            let sim = sim_of(
                "tiny-sim-plies",
                &cat_board(),
                &format!(r#","plies":{plies}"#),
            );
            do_sim_add(sim, &format!("[{CATS},{PASS}]")).unwrap();
            let answer = step(sim, r#"{"iterations":6,"prune":false}"#);
            assert_eq!(answer["iterations"], 6);
            for candidate in answer["candidates"].as_array().unwrap() {
                let counts = (candidate["plies"].as_array().unwrap().iter())
                    .map(|ply| ply["n"].as_u64().unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(counts.len(), plies);
                assert!(counts.windows(2).all(|w| w[0] >= w[1]), "{plies}");
                assert_eq!(Some(counts[0]), candidate["n"].as_u64(), "{plies}");
            }
            do_sim_free(sim).unwrap();
        }
    }

    // "plies": null is every ply to the game's end: each rollout as a sim of
    // more plies than any game here lasts plays it, the answer listing the
    // plies as far as the longest rollout reached
    #[test]
    fn a_sim_of_null_plies_rolls_out_to_the_games_end() {
        let sims = [r#","plies":null"#, r#","plies":1000"#]
            .map(|plies| sim_of("tiny-sim-many", &cat_board(), plies));
        for sim in sims {
            do_sim_add(sim, &format!("[{CATS},{PASS}]")).unwrap();
        }
        let [many, long] = sims.map(|sim| step(sim, r#"{"iterations":6,"prune":false}"#));
        for sim in sims {
            do_sim_free(sim).unwrap();
        }
        for (m, l) in (many["candidates"].as_array().unwrap().iter())
            .zip(long["candidates"].as_array().unwrap())
        {
            let (m_plies, l_plies) = (
                m["plies"].as_array().unwrap(),
                l["plies"].as_array().unwrap(),
            );
            assert!(m_plies.len() > move_picker::DEFAULT_NUM_SIM_PLIES);
            assert_eq!(m_plies[..], l_plies[..m_plies.len()]);
            assert!(l_plies[m_plies.len()..].iter().all(|ply| ply["n"] == 0));
            for stat in ["n", "win", "equity", "objective"] {
                assert_eq!(m[stat], l[stat], "{stat}");
            }
        }
        // the plies listed end at the longest rollout's last
        assert!(
            (many["candidates"].as_array().unwrap().iter()).any(|c| c["plies"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["n"]
                != 0)
        );
    }

    // "opponents_pass": every ply of the opponent's in a rollout is a pass
    #[test]
    fn a_sims_opponents_may_pass_every_turn() {
        let sims = [r#","opponents_pass":true"#, ""]
            .map(|more| sim_of("tiny-sim-opponents-pass", &cat_board(), more));
        for sim in sims {
            do_sim_add(sim, &format!("[{CATS},{PASS}]")).unwrap();
        }
        let [passing, playing] = sims.map(|sim| step(sim, r#"{"iterations":8,"prune":false}"#));
        for sim in sims {
            do_sim_free(sim).unwrap();
        }
        // the opponent's plies (the first after a candidate, then every other
        // one) with what each scored
        let theirs = |answer: &serde_json::Value| {
            (answer["candidates"].as_array().unwrap().iter())
                .flat_map(|c| c["plies"].as_array().unwrap().iter().step_by(2))
                .map(|ply| {
                    (
                        ply["n"].as_u64().unwrap(),
                        ply["score"]["mean"].as_f64().unwrap(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert!(
            theirs(&passing)
                .iter()
                .all(|&(n, score)| n > 0 && score == 0.0)
        );
        assert!(theirs(&playing).iter().any(|&(_, score)| score > 0.0));
    }

    // sim_set changes a live sim's plies and known tiles from the next
    // iteration on, its results kept, and leaves what it is not given
    #[test]
    fn a_live_sims_plies_and_known_tiles_change_its_results_kept() {
        let sim = cat_sim("tiny-sim-set");
        let before = step(sim, r#"{"iterations":4,"prune":false}"#);
        assert_eq!(report(&do_sim_set(sim, "{}").unwrap()), read_sim(sim));
        let set = report(&do_sim_set(sim, r#"{"plies":2}"#).unwrap());
        for (now, was) in (set["candidates"].as_array().unwrap().iter())
            .zip(before["candidates"].as_array().unwrap())
        {
            for stat in ["n", "win", "equity", "objective"] {
                assert_eq!(now[stat], was[stat], "{stat}");
            }
            assert_eq!(
                now["plies"].as_array().unwrap()[..],
                was["plies"].as_array().unwrap()[..2]
            );
        }
        let after = step(sim, r#"{"iterations":4,"prune":false}"#);
        assert_eq!(ns(&after), vec![8, 8]);
        for candidate in after["candidates"].as_array().unwrap() {
            let plies = candidate["plies"].as_array().unwrap();
            assert_eq!(plies.len(), 2);
            assert!(plies.iter().all(|ply| ply["n"] == 8));
        }
        // and those four iterations rolled out as a sim of two plies does
        let two = sim_of("tiny-sim-set", &cat_board(), r#","plies":2"#);
        do_sim_add(two, &format!("[{CATS},{PASS}]")).unwrap();
        step(two, r#"{"iterations":8,"prune":false}"#);
        {
            let sims = SIMS.lock().unwrap();
            for (now, fresh) in sims.live[&sim]
                .candidates
                .iter()
                .zip(&sims.live[&two].candidates)
            {
                assert_eq!(now.equities[4..], fresh.equities[4..]);
                assert_ne!(now.equities[..4], fresh.equities[..4]);
            }
        }
        do_sim_free(two).unwrap();

        // the opponent holds the known tiles each iteration from now on
        do_sim_set(sim, &format!(r#"{{"known":[{E},{E},0]}}"#)).unwrap();
        {
            let mut sims = SIMS.lock().unwrap();
            let sim = sims.live.get_mut(&sim).unwrap();
            for iteration in 9..=28 {
                sim.simmer.restore_prepared(&sim.pristine);
                sim.simmer.reseed(iteration_seed(sim.seed, iteration));
                sim.simmer.prepare_iteration();
                let rack = &sim.simmer.prepared_state().players[1].rack;
                for (tile, count) in [(E, 2), (0, 1)] {
                    assert!(
                        rack.iter().filter(|&&t| t == tile).count() >= count,
                        "{rack:?}"
                    );
                }
            }
        }
        // more than are unseen: refused, and nothing changes
        let read = read_sim(sim);
        let err = do_sim_set(sim, r#"{"plies":3,"known":[0,0,0]}"#).unwrap_err();
        assert!(err.starts_with("known: "), "{err}");
        assert_eq!(read_sim(sim), read);
        do_sim_free(sim).unwrap();
    }

    // a sim as a step of no iterations reads it
    fn read_sim(sim: u32) -> serde_json::Value {
        step(sim, r#"{"iterations":0,"prune":false}"#)
    }

    // the tests that infer, one at a time, since infer holds one set of tables
    // and a test reads which
    static INFER_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn one_infer_test() -> std::sync::MutexGuard<'static, ()> {
        INFER_TESTS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    // a late board to infer on: the watcher's rack (S E A T) and fourteen tiles
    // unseen, CATS's S among them, off the board, so the opponent's leaves are
    // few to weigh, the bag still holding seven, so they may exchange
    fn infer_board() -> String {
        late_board(&[S, E, A, T, A, C, E, E, S, T, B, D, F, G, H, I, I, J])
    }

    // CATS on the late board: S after its CAT
    const LATE_CATS: &str = r#"{"action":"play","down":false,"lane":2,"idx":6,"word":[0,0,0,19]}"#;

    // infer answers as wolges's own inference does, weights made to sum to 1: a
    // new handle each time, the count, and as many of the likeliest as asked
    // (the likeliest first; none unless asked); fuzz the caller's, never left
    // out; freed once
    #[test]
    fn infer_answers_as_wolges_infers() {
        let _one = one_infer_test();
        tiny_lexicon("tiny-infer");
        let rack = [S, E, A, T];
        let ask = |more: &str| {
            do_infer(&format!(
                r#"{{"rack":{rack:?},"board":{},"lexicon":"tiny-infer","leave":"noleave","rules":"CrosswordGame","play":{LATE_CATS}{more}}}"#,
                infer_board()
            ))
        };
        let fuzz = infer::FUZZ;
        let answer = ask(&format!(r#","fuzz":{fuzz},"list":{}"#, u32::MAX)).unwrap();

        let Tables {
            kwg,
            klv,
            game_config,
        } = tables("tiny-infer", "noleave", "CrosswordGame").unwrap();
        let mut kibitzer = kibitzer::Kibitzer::new();
        let board = serde_json::from_str::<Vec<Vec<i8>>>(&infer_board()).unwrap();
        kibitzer.prepare(&game_config, &rack, &board).unwrap();
        let play = serde_json::from_str::<SimPlay>(LATE_CATS)
            .unwrap()
            .to_play();
        let mut wolges = infer::kept_leaves_lean(
            &mut infer::InferTables::new(&game_config, &klv.klv).unwrap(),
            &mut movegen::KurniaMoveGenerator::<u32, false, false>::try_new(&game_config).unwrap(),
            &movegen::BoardSnapshot {
                board_tiles: &kibitzer.board_tiles,
                game_config: &game_config,
                kwg: &kwg.kwg,
                anagrams: None,
                rack_lengths: None,
                klv: &klv.klv,
            },
            &rack,
            &play,
            infer::FUZZ,
            1,
        )
        .unwrap();
        assert!(!wolges.is_empty());
        let total = wolges.iter().map(|(_, weight)| weight).sum::<f64>();
        for (_, weight) in &mut wolges {
            *weight /= total;
        }
        wolges.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let listed = (wolges.into_iter())
            .map(|(tiles, weight)| WeightedLeave { tiles, weight })
            .collect::<Vec<_>>();
        // compared as text, as a weight read back from JSON can be off in its
        // last place
        let handle_of = |answer: &str| report(answer)["handle"].as_u64().unwrap() as u32;
        let want = |handle: u32, n: usize| {
            serde_json::to_string(&InferAnswer {
                handle,
                count: listed.len(),
                leaves: listed[..n].to_vec(),
                tiles: Vec::new(),
            })
            .unwrap()
        };
        // the answer as text but its tiles (checked below); a weight read back
        // from JSON can be off in its last place, so the text is compared
        let but_tiles = |answer: &str| {
            let cut = answer.find(r#","tiles":"#).unwrap();
            format!("{}}}", &answer[..cut])
        };
        let first = handle_of(&answer);
        assert_eq!(but_tiles(&answer), but_tiles(&want(first, listed.len())));
        let three = ask(&format!(r#","fuzz":{fuzz},"list":3"#)).unwrap();
        assert_eq!(but_tiles(&three), but_tiles(&want(first + 1, 3)));
        let none = ask(&format!(r#","fuzz":{fuzz}"#)).unwrap();
        assert_eq!(but_tiles(&none), but_tiles(&want(first + 2, 0)));
        // each tile over every leave, however many were listed: the tiles
        // unseen after CATS (its S on the board), a rack of 7 of them; a tile
        // unseen nowhere is {0, 0}, and the prior is the deal's alone
        let mut unseen = kibitzer.available_tally.clone();
        unseen[S as usize] -= 1;
        let total = unseen.iter().map(|&count| count as usize).sum::<usize>();
        let tiles = infer_tiles(
            &unseen,
            7.min(total),
            listed.iter().map(|leave| (&leave.tiles[..], leave.weight)),
        );
        for answer in [&answer, &three, &none] {
            let found = report(answer)["tiles"].as_array().unwrap().clone();
            assert_eq!(found.len(), unseen.len());
            for (tile, (found, want)) in found.iter().zip(&tiles).enumerate() {
                let held = found["held"].as_f64().unwrap();
                let prior = found["prior"].as_f64().unwrap();
                assert!((held - want.held).abs() < 1e-9, "{tile}: {found}");
                assert_eq!(prior, want.prior, "{tile}");
                let left = total - unseen[tile] as usize;
                let deal = 1.0 - choose(left, 7.min(total)) / choose(total, 7.min(total));
                assert!((prior - deal).abs() < 1e-12, "{tile}");
                if unseen[tile] == 0 {
                    assert_eq!((held, prior), (0.0, 0.0), "{tile}");
                }
            }
        }
        for handle in first..first + 3 {
            do_infer_free(handle).unwrap();
        }
        let err = do_infer_free(first).unwrap_err();
        assert!(err.starts_with(&format!("inference {first}: ")), "{err}");

        // fuzz is the caller's to give: never left out, none below 0
        for more in ["", r#","fuzz":-1"#] {
            let err = ask(more).unwrap_err();
            assert!(err.contains("fuzz"), "{err}");
        }

        // a play with a bonus or challenged off is refused: neither says what
        // was kept
        for more in [r#""bonus":5"#, r#""challenged_off":true"#] {
            let err = do_infer(&format!(
                r#"{{"rack":{rack:?},"board":{},"lexicon":"tiny-infer","leave":"noleave","rules":"CrosswordGame","play":{},{more}}},"fuzz":{fuzz}}}"#,
                infer_board(),
                LATE_CATS.strip_suffix('}').unwrap()
            ))
            .unwrap_err();
            assert!(err.starts_with("play: "), "{err}");
        }
    }

    // a leave table of these leaves (in order) and values, laid out as a klv2
    fn leave_table(leaves: &[(&[u8], f32)]) -> CachedKlv {
        let words = (leaves.iter())
            .map(|&(tiles, _)| tiles.into())
            .collect::<Vec<bites::Bites>>();
        let graph = build::build(
            build::BuildContent::DawgOnly,
            build::BuildLayout::Wolges,
            build::BuildOrder::Sorted,
            &words,
        )
        .unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&((graph.len() / 4) as u32).to_le_bytes());
        bytes.extend_from_slice(&graph);
        bytes.extend_from_slice(&(leaves.len() as u32).to_le_bytes());
        for &(_, value) in leaves {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        CachedKlv::new(
            Klv::from_reader::<kwg::Node22>(bytes.len(), |at, piece| {
                piece.copy_from_slice(&bytes[at..at + piece.len()])
            })
            .unwrap(),
        )
        .unwrap()
    }

    // a leave table cached again under its name answers infer as it is now, not
    // as the inference tables made from the one before it would; another
    // table's tables, or other rules', take the place of those held; evicted, a
    // table takes its tables along
    #[test]
    fn infer_answers_from_a_leave_table_as_it_is_cached_now() {
        let _one = one_infer_test();
        tiny_lexicon("tiny-infer-again");
        tiny_lexicon_for("tiny-infer-again-jumbled", true);
        // a lexicon and its rules
        const ENGLISH: (&str, &str) = ("tiny-infer-again", "CrosswordGame");
        const JUMBLED: (&str, &str) = ("tiny-infer-again-jumbled", "WordSmog");
        // every leave inferred, the handle freed
        let ask = |leave: &str, (lexicon, rules): (&str, &str)| {
            let answer = report(
                &do_infer(&format!(
                    r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"{lexicon}","leave":"{leave}","rules":"{rules}","play":{LATE_CATS},"fuzz":{},"list":{}}}"#,
                    infer_board(),
                    infer::FUZZ,
                    u32::MAX
                ))
                .unwrap(),
            );
            do_infer_free(answer["handle"].as_u64().unwrap() as u32).unwrap();
            answer["leaves"].clone()
        };
        let made = |leave: &str, (_, rules): (&str, &str)| {
            (CACHED_INFER.lock().unwrap().as_ref())
                .is_some_and(|held| held.leave == leave && held.rules == rules)
        };
        let table = || CachedKlv::new(Klv::empty().unwrap()).unwrap();
        cache_klv("tiny-infer-again".into(), table()).unwrap();
        let before = ask("tiny-infer-again", ENGLISH);
        assert!(made("tiny-infer-again", ENGLISH));
        // the same table under other rules
        ask("tiny-infer-again", JUMBLED);
        assert!(made("tiny-infer-again", JUMBLED));
        // an E alone worth 100 points as a leave: a rack holding one would
        // sooner exchange the rest and keep it than play CATS
        let table = || leave_table(&[(&[E], 100.0)]);
        cache_klv("tiny-infer-again".into(), table()).unwrap();
        assert!(!made("tiny-infer-again", JUMBLED));
        let after = ask("tiny-infer-again", ENGLISH);
        assert_ne!(after, before);
        cache_klv("tiny-infer-fresh".into(), table()).unwrap();
        assert_eq!(after, ask("tiny-infer-fresh", ENGLISH));
        assert!(made("tiny-infer-fresh", ENGLISH) && !made("tiny-infer-again", ENGLISH));
        assert!(evict_klv("tiny-infer-fresh".into()).unwrap());
        assert!(!made("tiny-infer-fresh", ENGLISH));
    }

    // with the bag empty there is nothing to infer: the mover holds every tile
    // the watcher cannot see
    #[test]
    fn infer_with_an_empty_bag_answers_no_leaves() {
        let _one = one_infer_test();
        tiny_lexicon("tiny-infer-late");
        let (rack, others) = ([S, E, A, T], [A, T, E]);
        let racks = rack.iter().chain(&others).copied().collect::<Vec<_>>();
        let answer = report(
            &do_infer(&format!(
                r#"{{"rack":{rack:?},"board":{},"lexicon":"tiny-infer-late","leave":"noleave","rules":"CrosswordGame","play":{PASS},"fuzz":{},"list":{}}}"#,
                late_board(&racks),
                infer::FUZZ,
                u32::MAX
            ))
            .unwrap(),
        );
        let handle = answer["handle"].clone();
        // no leaves: each tile's held is its prior, and with three tiles unseen
        // and the bag empty the rack holds them all, A, T and E for certain
        let tiles = (0..27)
            .map(|tile| {
                let p = if [A, T, E].contains(&tile) { 1.0 } else { 0.0 };
                serde_json::json!({"held": p, "prior": p})
            })
            .collect::<Vec<_>>();
        assert_eq!(
            answer,
            serde_json::json!({"handle": handle, "count": 0, "leaves": [], "tiles": tiles})
        );
        do_infer_free(handle.as_u64().unwrap() as u32).unwrap();
    }

    // a sim given an inference's handle deals its opponent one of its leaves
    // each iteration, as one given the same leaves as a list does, and keeps
    // them once the handle is freed; sim_set takes a handle too. The handle
    // goes with neither known tiles nor kept leaves, and its leaves must fit
    // what the sim's opponent may hold
    #[test]
    fn a_sim_deals_its_opponent_one_of_an_inferences_leaves() {
        let _one = one_infer_test();
        tiny_lexicon("tiny-infer-sim");
        let inferred = || {
            let answer = report(
                &do_infer(&format!(
                    r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-infer-sim","leave":"noleave","rules":"CrosswordGame","play":{CATS},"fuzz":{}}}"#,
                    cat_board(),
                    infer::FUZZ
                ))
                .unwrap(),
            );
            assert!(answer["count"].as_u64().unwrap() > 0);
            answer["handle"].as_u64().unwrap() as u32
        };
        // the opponent's rack on each of these iterations
        let dealt = |sim: u32, iterations: std::ops::RangeInclusive<u64>| {
            let mut sims = SIMS.lock().unwrap();
            let sim = sims.live.get_mut(&sim).unwrap();
            (iterations.map(|iteration| {
                sim.simmer.restore_prepared(&sim.pristine);
                sim.simmer.reseed(iteration_seed(sim.seed, iteration));
                sim.simmer.prepare_iteration();
                sim.simmer.prepared_state().players[1].rack.clone()
            }))
            .collect::<Vec<_>>()
        };
        let handle = inferred();
        let leaves = INFERENCES.lock().unwrap().live[&handle].clone();
        let from_handle = sim_of(
            "tiny-infer-sim",
            &cat_board(),
            &format!(r#","inferred":{handle}"#),
        );
        do_infer_free(handle).unwrap();
        // the sim and this test share the leaves; none were copied
        assert_eq!(std::sync::Arc::strong_count(&leaves), 2);
        let listed = (0..leaves.len())
            .map(|i| (leaves.leave(i).to_vec(), leaves.weight(i)))
            .collect::<Vec<_>>();
        let from_list = sim_of("tiny-infer-sim", &cat_board(), "");
        let plain = dealt(from_list, 1..=20);
        (SIMS.lock().unwrap().live.get_mut(&from_list).unwrap())
            .simmer
            .set_kept_leaves(1, &listed);
        let racks = dealt(from_handle, 1..=20);
        assert_eq!(dealt(from_list, 1..=20), racks);
        assert_ne!(plain, racks);
        // sim_set takes a handle: a sim with none then deals as one made with it
        let again = inferred();
        let set = sim_of("tiny-infer-sim", &cat_board(), "");
        do_sim_set(set, &format!(r#"{{"inferred":{again}}}"#)).unwrap();
        assert_eq!(dealt(set, 1..=20), racks);
        for sim in [from_handle, from_list, set] {
            do_sim_free(sim).unwrap();
        }

        let err = do_sim_set(from_handle, &format!(r#"{{"inferred":{again}}}"#)).unwrap_err();
        assert!(err.starts_with("sim "), "{err}");
        for (more, said) in [
            (format!(r#","inferred":{handle}"#), "inferred: inference "),
            (
                format!(r#","known":[{E}],"inferred":{again}"#),
                "known, inferred",
            ),
            (
                format!(r#","kept":[{{"tiles":[0],"weight":1}}],"inferred":{again}"#),
                "kept, inferred",
            ),
            // an opponent of five tiles cannot hold a leave of six
            (
                format!(r#","rack_sizes":[4,5],"inferred":{again}"#),
                "inferred: leave 0 has 6 tiles",
            ),
        ] {
            let err = do_sim_new(&format!(
                r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-infer-sim","leave":"noleave","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0,"plies":4{more}}}"#,
                cat_board()
            ))
            .unwrap_err();
            assert!(err.starts_with(said), "{err}");
        }
        // a mover holding both blanks leaves the opponent none to keep
        let err = do_sim_new(&format!(
            r#"{{"rack":[0,0,{S},{E},{A},{T}],"board":{},"lexicon":"tiny-infer-sim","leave":"noleave","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0,"plies":4,"inferred":{again}}}"#,
            cat_board()
        ))
        .unwrap_err();
        assert!(err.contains("is not all among the tiles unseen"), "{err}");
        do_infer_free(again).unwrap();
    }

    // a sim given kept leaves deals its opponent one of them each iteration,
    // chosen by weight, as sim_set does too; known and kept do not go together
    #[test]
    fn a_sim_deals_its_opponent_one_of_its_kept_leaves() {
        let kept =
            format!(r#""kept":[{{"tiles":[{E},{E}],"weight":3}},{{"tiles":[0],"weight":1}}]"#);
        let handle = sim_of("tiny-sim-kept", &cat_board(), &format!(",{kept}"));
        let dealt = |handle: u32, iterations: std::ops::RangeInclusive<u64>| {
            let mut sims = SIMS.lock().unwrap();
            let sim = sims.live.get_mut(&handle).unwrap();
            let mut counts = [0, 0];
            for iteration in iterations {
                sim.simmer.restore_prepared(&sim.pristine);
                sim.simmer.reseed(iteration_seed(sim.seed, iteration));
                sim.simmer.prepare_iteration();
                let rack = &sim.simmer.prepared_state().players[1].rack;
                let es = rack.iter().filter(|&&t| t == E).count() >= 2;
                let blank = rack.contains(&0);
                assert!(es || blank, "{rack:?}");
                counts[0] += es as u32;
                counts[1] += blank as u32;
            }
            counts
        };
        let counts = dealt(handle, 1..=40);
        assert!(counts[0] > 0 && counts[1] > 0, "{counts:?}");
        // sim_set to the blank alone, the E's weighing nothing
        do_sim_set(
            handle,
            &format!(r#"{{"kept":[{{"tiles":[{E},{E}],"weight":0}},{{"tiles":[0],"weight":1}}]}}"#),
        )
        .unwrap();
        assert_eq!(dealt(handle, 41..=60)[1], 20);
        do_sim_free(handle).unwrap();

        tiny_lexicon("tiny-sim-kept-bad");
        for (more, said) in [
            (format!(r#","known":[{E}],{kept}"#), "known, kept"),
            (
                r#","kept":[{"tiles":[0],"weight":-1}]"#.into(),
                "kept[0]: weight",
            ),
            (
                r#","kept":[{"tiles":[0,0,0],"weight":1}]"#.into(),
                "kept[0]: ",
            ),
            (
                r#","kept":[{"tiles":[0],"weight":0}]"#.into(),
                "kept: weights",
            ),
            (
                r#","kept":[{"tiles":[0],"weight":1e308},{"tiles":[0],"weight":1e308}]"#.into(),
                "kept: weights",
            ),
        ] {
            let err = do_sim_new(&format!(
                r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-sim-kept-bad","leave":"noleave","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0,"plies":4{more}}}"#,
                cat_board()
            ))
            .unwrap_err();
            assert!(err.starts_with(said), "{err}");
        }
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
        let answer = report(&do_sim_step(sim, r#"{"ms":3.5,"prune":false}"#, now).unwrap());
        assert_eq!(answer["stepped"], 3);
        // iterations bound it too, whichever ends first
        let answer =
            report(&do_sim_step(sim, r#"{"ms":1000,"iterations":2,"prune":false}"#, now).unwrap());
        assert_eq!(answer["stepped"], 2);
        assert!(do_sim_step(sim, r#"{}"#, now).is_err());
        assert!(do_sim_step(sim, r#"{"ms":-1,"prune":false}"#, now).is_err());
        do_sim_free(sim).unwrap();
    }

    // scores are points: a thousand of them ahead wins, behind loses; and a sim
    // must be told them
    #[test]
    fn a_sims_scores_are_points() {
        tiny_lexicon("tiny-sim-scores");
        let sim_new = |scores: &str| {
            do_sim_new(&format!(
                r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-sim-scores","leave":"noleave","rules":"CrosswordGame","turn":0,"zero_turns":0,"pass_turns":0,"plies":4,"seed":7{scores}}}"#,
                cat_board()
            ))
        };
        for (scores, ahead) in [
            (r#","scores":[1000,0]"#, true),
            (r#","scores":[0,1000]"#, false),
        ] {
            let sim = sim_new(scores).unwrap();
            do_sim_add(sim, &format!("[{PASS}]")).unwrap();
            let answer = step(sim, r#"{"iterations":4,"prune":false}"#);
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
                r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-sim-scores","leave":"noleave","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0,"plies":4}}"#,
                cat_board()
            )
            .replace(&format!(r#","{field}":0"#), "");
            let err = do_sim_new(&req).unwrap_err();
            assert!(err.contains(&format!("missing field `{field}`")), "{err}");
        }
        // so are the plies, which may be null but not left out
        let req = |plies: &str| {
            format!(
                r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-sim-scores","leave":"noleave","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0{plies}}}"#,
                cat_board()
            )
        };
        let err = do_sim_new(&req("")).unwrap_err();
        assert!(err.contains("missing field `plies`"), "{err}");
        do_sim_free(do_sim_new(&req(r#","plies":null"#)).unwrap()).unwrap();
    }

    // stitched_z is wolges's at whole iterations from 1 and an alpha between 0
    // and 1, and refuses anything else
    #[test]
    fn stitched_z_is_wolges_own_within_its_bounds() {
        for (iterations, alpha) in [(1.0, 0.05), (1000.0, 0.05), (87057.0, 0.01), (1e15, 0.5)] {
            assert_eq!(
                do_stitched_z(iterations, alpha).unwrap(),
                move_picker::stitched_z(iterations as u64, alpha)
            );
        }
        for (iterations, alpha) in [
            (0.0, 0.05),
            (1.5, 0.05),
            (f64::NAN, 0.05),
            (f64::INFINITY, 0.05),
            (u64::MAX as f64, 0.05),
            (16.0, 0.0),
            (16.0, 1.0),
            (16.0, -0.5),
            (16.0, f64::NAN),
        ] {
            assert!(
                do_stitched_z(iterations, alpha).is_err(),
                "{iterations} {alpha}"
            );
        }
    }

    // a sim's spread and leaves are its equity's two parts: on every rollout
    // they sum to it, so their means do too, a lead at the start counted in
    // neither; with a table that values leaves, its leaves are not all 0
    #[test]
    fn a_sims_spread_and_leaves_sum_to_its_equity() {
        tiny_lexicon("tiny-sim-parts");
        // a leave table of one tile each, in the graph's sorted order
        let leaves: [(&[u8], f32); 5] = [
            (&[A], 1.5),
            (&[C], -2.0),
            (&[E], 3.25),
            (&[S], 8.0),
            (&[T], 0.75),
        ];
        let words = leaves
            .iter()
            .map(|&(tiles, _)| tiles.into())
            .collect::<Vec<bites::Bites>>();
        let graph = build::build(
            build::BuildContent::DawgOnly,
            build::BuildLayout::Wolges,
            build::BuildOrder::Sorted,
            &words,
        )
        .unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&((graph.len() / 4) as u32).to_le_bytes());
        bytes.extend_from_slice(&graph);
        bytes.extend_from_slice(&(leaves.len() as u32).to_le_bytes());
        for &(_, value) in &leaves {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let table = Klv::from_reader::<kwg::Node22>(bytes.len(), |at, piece| {
            piece.copy_from_slice(&bytes[at..at + piece.len()])
        })
        .unwrap();
        cache_klv(
            "tiny-sim-parts-leaves".into(),
            CachedKlv::new(table).unwrap(),
        )
        .unwrap();
        let sim = do_sim_new(&format!(
            r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-sim-parts","leave":"tiny-sim-parts-leaves","rules":"CrosswordGame","scores":[100,40],"turn":0,"zero_turns":0,"pass_turns":0,"plies":2,"seed":7}}"#,
            cat_board()
        ))
        .unwrap();
        do_sim_add(sim, &format!("[{CATS},{PASS}]")).unwrap();
        let answer = step(sim, r#"{"iterations":32,"prune":false}"#);
        let candidates = answer["candidates"].as_array().unwrap();
        assert_eq!(candidates.len(), 2);
        let mut valued = 0;
        for candidate in candidates {
            let mean = |field: &str| candidate[field]["mean"].as_f64().unwrap();
            let (equity, spread, leaves) = (mean("equity"), mean("spread"), mean("leaves"));
            assert!(
                (spread + leaves - equity).abs() < 1e-9,
                "{spread} + {leaves} against {equity}"
            );
            valued += (leaves != 0.0) as usize;
        }
        assert!(valued > 0, "every candidate's leaves 0");
        do_sim_free(sim).unwrap();
    }

    // a sim knows the passes in a row so far: under German rules, where four
    // passes in a row end the game, after three a pass ends it at once, and no
    // ply follows
    #[test]
    fn a_sims_passes_so_far_can_end_its_game() {
        tiny_lexicon("tiny-sim-passes");
        let plies_reached = |pass_turns: u16| {
            let sim = do_sim_new(&format!(
                r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-sim-passes","leave":"noleave","rules":"CrosswordGame/german","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":{pass_turns},"plies":4,"seed":7}}"#,
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
        let answer = step(sim, r#"{"iterations":2,"prune":false}"#);
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
                r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-sim-known-bad","leave":"noleave","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0,"plies":4,"known":{known}}}"#,
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

        // a play the rack cannot make refuses the whole change, naming it, and
        // nothing joins
        let off = r#"{"action":"play","down":true,"lane":8,"idx":6,"word":[26,0]}"#;
        let err = do_sim_add(sim, &format!("[{SCAT},{off}]")).unwrap_err();
        assert!(err.starts_with("plays[1]: "), "{err}");
        assert_eq!(
            ns(&step(sim, r#"{"iterations":0,"prune":false}"#)),
            [5, 5, 1]
        );

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

    // a sim takes what quackle's does: a phony, which stands as one no one
    // challenged would; a play with a challenge's bonus; a play challenged off
    // the board. That last scores nothing. It rolls out as a pass does but for
    // its turn. Each is a candidate of its own.
    #[test]
    fn a_sim_takes_a_phony_a_bonus_and_a_play_challenged_off() {
        let sim = cat_sim("tiny-sim-phony");
        // E below the T of CAT: TE down, a word the tiny lexicon lacks
        let te = r#"{"action":"play","down":true,"lane":8,"idx":7,"word":[0,5]"#;
        let (phony, plus5, off) = (
            format!("{te}}}"),
            format!(r#"{te},"bonus":5}}"#),
            format!(r#"{te},"challenged_off":true}}"#),
        );
        let answer = report(&do_sim_add(sim, &format!("[{phony},{plus5},{off}]")).unwrap());
        for held in answer.as_array().unwrap() {
            assert_eq!(held["was"], "absent");
        }
        let score = answer[0]["play"]["score"].as_i64().unwrap();
        assert!(score > 0);
        assert_eq!(answer[1]["play"]["score"].as_i64().unwrap(), score + 5);
        assert_eq!(answer[2]["play"]["score"], 0);
        assert_eq!(answer[2]["challenged_off"], true);
        assert!(answer[0].get("challenged_off").is_none());
        // challenged off, it keeps the rack whole: a pass's static equity
        let pass = report(&do_sim_add(sim, &format!("[{PASS}]")).unwrap());
        assert_eq!(pass[0]["was"], "running");
        assert_eq!(answer[2]["play"]["equity"], pass[0]["play"]["equity"]);
        // a play challenged off scores nothing, so takes no bonus
        let both = format!(r#"{te},"bonus":5,"challenged_off":true}}"#);
        let err = do_sim_add(sim, &format!("[{both}]")).unwrap_err();
        assert!(err.starts_with("plays[0]: "), "{err}");

        // CATS, the pass, the phony, it with its bonus, it challenged off
        let stepped = step(sim, r#"{"iterations":6,"prune":false}"#);
        assert_eq!(ns(&stepped), [6, 6, 6, 6, 6]);
        let c = stepped["candidates"].as_array().unwrap();
        assert_eq!(c[4]["challenged_off"], true);
        // on the same draws, challenged off rolls out as the pass does
        assert_eq!(c[4]["plies"], c[1]["plies"]);
        assert_eq!(c[4]["equity"], c[1]["equity"]);
        // and the bonus is five more points of spread on every rollout
        let mean = |i: usize| c[i]["equity"]["mean"].as_f64().unwrap();
        assert!((mean(3) - mean(2) - 5.0).abs() < 1e-9, "{c:?}");

        // the flag is part of which candidate a play names
        let removed = report(&do_sim_change(sim, &format!("[{off}]"), SimChange::Remove).unwrap());
        assert_eq!(removed[0]["was"], "running");
        let left = step(sim, r#"{"iterations":0,"prune":false}"#);
        assert_eq!(ns(&left), [6, 6, 6, 6]);
        assert!(
            (left["candidates"].as_array().unwrap().iter())
                .all(|c| c.get("challenged_off").is_none())
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
        let answer = step(sim, &format!(r#"{{"iterations":256,{}}}"#, pruning(0.0)));
        assert_eq!(states(&answer), ["running", "pruned"]);
        let n = ns(&answer);
        assert_eq!(n[0], 256);
        assert!(
            n[1] < 256 && n[1].is_multiple_of(move_picker::PRUNE_CADENCE),
            "{n:?}"
        );
        // the caller's settings, not wolges's: looks every 3 iterations prune
        // the pass at a multiple of 3 that no look every 16 would have had, and
        // a z no difference reaches prunes nothing
        let settings = |every: u64, z: f64| {
            let sim = cat_sim("tiny-sim-prune");
            let answer = step(
                sim,
                &format!(
                    r#"{{"iterations":256,"prune":true,"prune_every":{every},"prune_z":{z:?},"prune_decided":0}}"#
                ),
            );
            do_sim_free(sim).unwrap();
            (states(&answer), ns(&answer))
        };
        let z = move_picker::paired_z(move_picker::DEFAULT_NUM_SIM_ITERS);
        let (states3, n3) = settings(3, z);
        assert_eq!(states3, ["running", "pruned"]);
        assert!(
            n3[1].is_multiple_of(3) && !n3[1].is_multiple_of(16),
            "{n3:?}"
        );
        assert_eq!(settings(16, 1000.0).0, ["running", "running"]);
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
        assert_eq!(step(sim, r#"{"iterations":5,"prune":false}"#)["stepped"], 0);
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
            challenged_off: false,
            static_equity: 0.0,
            state: SimState::Running,
            first_iter: 1,
            wins: wins.into(),
            equities: equities.into(),
            objective: stats::Stats::new(),
            equity: stats::Stats::new(),
            spread: stats::Stats::new(),
            leaves: stats::Stats::new(),
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
        // decided: within the margin a step gives of certain
        let d = 0.001;
        assert_eq!(outcome(1.0 - d, d), Outcome::Won);
        assert_eq!(outcome(d, d), Outcome::Lost);
        assert_eq!(outcome(1.0 - 2.0 * d, d), Outcome::Open);
        assert_eq!(outcome(2.0 * d, d), Outcome::Open);
        assert_eq!(outcome(1.0 - d, 0.0), Outcome::Open);
        assert_eq!(outcome(1.0, 0.0), Outcome::Won);
        assert_eq!(outcome(0.0, 0.0), Outcome::Lost);
        assert!(by_equity(1.0 - d, 1.0, d) && by_equity(0.0, d, d));
        assert!(!by_equity(1.0 - d, 1.0, 0.0));
        assert!(!by_equity(0.6, 1.0, d) && !by_equity(0.0, 1.0, d) && !by_equity(0.5, 0.5, d));
        // a step says whether it prunes, and to prune gives every setting, each
        // in range
        let asked = |json: &str| pruning_of(&serde_json::from_str(json).unwrap());
        assert!(asked(r#"{"prune":false}"#).unwrap().is_none());
        let given = asked(r#"{"prune":true,"prune_every":16,"prune_z":3.5,"prune_decided":0.5}"#)
            .unwrap()
            .unwrap();
        assert_eq!((given.every, given.z, given.decided), (16, 3.5, 0.5));
        for (json, said) in [
            (r#""prune_z":3.5,"prune_decided":0.001"#, "prune_every: "),
            (r#""prune_every":16,"prune_decided":0.001"#, "prune_z: "),
            (r#""prune_every":16,"prune_z":3.5"#, "prune_decided: "),
            (
                r#""prune_every":0,"prune_z":3.5,"prune_decided":0.001"#,
                "prune_every: ",
            ),
            (
                r#""prune_every":16,"prune_z":-1,"prune_decided":0.001"#,
                "prune_z: ",
            ),
            (
                r#""prune_every":16,"prune_z":3.5,"prune_decided":0.6"#,
                "prune_decided: ",
            ),
            (
                r#""prune_every":16,"prune_z":3.5,"prune_decided":-0.1"#,
                "prune_decided: ",
            ),
        ] {
            let err = asked(&format!(r#"{{"prune":true,{json}}}"#)).unwrap_err();
            assert!(err.starts_with(said), "{json}: {err}");
        }
        assert!(serde_json::from_str::<SimStepRequest>(r#"{"iterations":1}"#).is_err());

        let z = move_picker::paired_z(move_picker::DEFAULT_NUM_SIM_ITERS);
        // 32 iterations alternating between two values
        let two = |a: f64, b: f64| (0..32).map(|i| [a, b][i % 2]).collect::<Vec<_>>();
        let standing = |candidates: &[SimCandidate]| {
            (candidates.iter())
                .map(|c| c.state.name())
                .collect::<Vec<_>>()
        };
        // two games won within the margin: the leader is the one of the higher
        // equity, though the other's chance to win is higher; with no margin
        // neither is won, and the leader is the higher chance to win
        let candidates = [
            rolled(&two(0.9995, 0.9995), &two(30.0, 30.0)),
            rolled(&two(0.9999, 0.9999), &two(20.0, 20.0)),
        ];
        assert!(std::ptr::eq(
            leader(&candidates, d).unwrap(),
            &candidates[0]
        ));
        assert!(std::ptr::eq(
            leader(&candidates, 0.0).unwrap(),
            &candidates[1]
        ));
        // two won games and an open one: the leader is the won one of the
        // higher equity, the other won one is behind it in equity, and the open
        // one, though its equity is the highest, is behind it in the chance to
        // win
        let mut candidates = [
            rolled(&two(1.0, 1.0), &two(20.0, 22.0)),
            rolled(&two(1.0, 1.0), &two(30.0, 33.0)),
            rolled(&two(0.5, 0.7), &two(90.0, 95.0)),
        ];
        assert!(std::ptr::eq(
            leader(&candidates, 0.0).unwrap(),
            &candidates[1]
        ));
        prune_behind(&mut candidates, z, 0.0);
        assert_eq!(standing(&candidates), ["pruned", "running", "pruned"]);
        // every game lost: the leader is the one of the highest equity, and the
        // others are behind it in equity
        let mut candidates = [
            rolled(&two(0.0, 0.0), &two(-90.0, -95.0)),
            rolled(&two(0.0, 0.0), &two(-20.0, -22.0)),
        ];
        prune_behind(&mut candidates, z, 0.0);
        assert_eq!(standing(&candidates), ["pruned", "running"]);
        // open games: the leader is the one of the highest chance to win, and a
        // play of more equity but less chance to win is behind it; a play never
        // apart from it in the chance to win is not
        let mut candidates = [
            rolled(&two(0.5, 0.7), &two(90.0, 95.0)),
            rolled(&two(0.6, 0.8), &two(10.0, 12.0)),
            rolled(&two(0.6, 0.8), &two(0.0, 1.0)),
        ];
        prune_behind(&mut candidates, z, 0.0);
        assert_eq!(standing(&candidates), ["pruned", "running", "running"]);
        // a paused candidate is not the leader, and is not pruned
        let mut candidates = [
            rolled(&two(0.6, 0.8), &two(10.0, 12.0)),
            rolled(&two(0.9, 0.9), &two(10.0, 12.0)),
        ];
        candidates[1].state = SimState::Paused;
        prune_behind(&mut candidates, z, 0.0);
        assert_eq!(standing(&candidates), ["running", "paused"]);

        // a sim far ahead: every rollout's chance to win is within 0.001 of 1,
        // though not 1 itself. Given that margin, CATS and the pass are compared
        // by equity, and the pass, which scores nothing now, is pruned; by
        // default neither game is decided, and chances to win this near each
        // other prune neither
        tiny_lexicon("tiny-sim-decided");
        let far_ahead = || {
            let sim = do_sim_new(&format!(
                r#"{{"rack":[{S},{E},{A},{T}],"board":{},"lexicon":"tiny-sim-decided","leave":"noleave","rules":"CrosswordGame","scores":[1000,0],"turn":0,"zero_turns":0,"pass_turns":0,"plies":4,"seed":7}}"#,
                cat_board()
            ))
            .unwrap();
            do_sim_add(sim, &format!("[{CATS},{PASS}]")).unwrap();
            sim
        };
        let sim = far_ahead();
        let err = do_sim_step(
            sim,
            &format!(r#"{{"iterations":1,{}}}"#, pruning(0.6)),
            || 0.0,
        )
        .unwrap_err();
        assert!(err.starts_with("prune_decided: "), "{err}");
        let answer = step(sim, &format!(r#"{{"iterations":256,{}}}"#, pruning(0.001)));
        assert_eq!(states(&answer), ["running", "pruned"]);
        for c in 0..2 {
            let win = answer["candidates"][c]["win"]["mean"].as_f64().unwrap();
            assert_eq!(outcome(win, 0.001), Outcome::Won, "{win}");
            assert_eq!(outcome(win, 0.0), Outcome::Open, "{win}");
        }
        do_sim_free(sim).unwrap();
        let sim = far_ahead();
        let answer = step(sim, &format!(r#"{{"iterations":256,{}}}"#, pruning(0.0)));
        assert_eq!(states(&answer), ["running", "running"]);
        assert_eq!(ns(&answer), [256, 256]);
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
            step(sim, r#"{"iterations":0,"prune":false}"#)["candidates"][0]
                .get("against")
                .is_none()
        );
        // a play not in the sim: refused before any step
        let err = do_sim_step(
            sim,
            &format!(r#"{{"iterations":3,"prune":false,"against":{SCAT}}}"#),
            || 0.0,
        )
        .unwrap_err();
        assert_eq!(err, "against: not in the sim");
        assert_eq!(
            step(sim, r#"{"iterations":0,"prune":false}"#)["iterations"],
            10
        );
        do_sim_free(sim).unwrap();
    }

    #[test]
    fn a_sim_keeps_its_graph_evicted_and_a_freed_sim_is_gone() {
        let sim = cat_sim("tiny-sim-free");
        let before = step(sim, r#"{"iterations":4,"prune":false}"#);
        // the name can go while the sim lives: the sim keeps the graph it was
        // made with and steps on it as before
        assert!(evict_kwg("tiny-sim-free".into()).unwrap());
        let after = step(sim, r#"{"iterations":4,"prune":false}"#);
        assert_eq!(after["iterations"], 8);
        assert_eq!(
            before["candidates"].as_array().unwrap().len(),
            after["candidates"].as_array().unwrap().len()
        );
        // a sim of a lexicon no longer cached
        let err = do_sim_new(&format!(
            r#"{{"rack":[{S}],"board":{},"lexicon":"tiny-sim-free","leave":"noleave","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0,"plies":4}}"#,
            cat_board()
        ))
        .unwrap_err();
        assert_eq!(err, "missing kwg");
        do_sim_free(sim).unwrap();
        for err in [
            do_sim_step(sim, r#"{"iterations":1,"prune":false}"#, || 0.0).unwrap_err(),
            do_sim_add(sim, &format!("[{PASS}]")).unwrap_err(),
            do_sim_free(sim).unwrap_err(),
        ] {
            assert!(err.contains("freed"), "{err}");
        }
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
        report(&do_endgame_step(handle, req, frozen, held_bytes).unwrap())
    }

    fn with_draws(handle: u32, answer: &serde_json::Value) -> Vec<serde_json::Value> {
        (0..answer["candidates"].as_array().unwrap().len())
            .map(|i| {
                let one = endgame_step(handle, &format!(r#"{{"ms":0,"draws_of":{i}}}"#));
                one["candidates"][i].clone()
            })
            .collect()
    }

    // the bytes held count every byte live, so never fewer than a buffer kept
    #[test]
    fn the_wasm_says_how_many_bytes_it_holds() {
        let kept = std::hint::black_box(vec![1u8; 1 << 20]);
        assert!(memory_held() >= kept.len() as f64);
        drop(kept);
    }

    // an empty bag asked for every first move's value: each exact, best first,
    // the first the answer's play and value, none worth more; with tiles in the
    // bag the asking is refused (candidates hold them already)
    #[test]
    fn an_endgame_values_every_first_move_when_asked() {
        let (rack, others) = ([S, E, A, T], [A, T, E]);
        let handle = endgame_of("tiny-end-first", &rack, &others, "").unwrap();
        let plain = endgame_step(handle, r#"{"ms":60000}"#);
        assert!(plain.get("candidates").is_none(), "{plain}");
        let answer = endgame_step(handle, r#"{"ms":0,"first_moves_ms":60000}"#);
        let candidates = answer["candidates"].as_array().unwrap();
        assert!(candidates.len() > 1, "{answer}");
        assert_eq!(candidates[0]["value"], answer["value"]);
        assert!(candidates.iter().all(|c| c["exact"] == true), "{answer}");
        let best = answer["value"].as_f64().unwrap();
        assert!(
            candidates
                .iter()
                .all(|c| c["value"].as_f64().unwrap() <= best),
            "{answer}"
        );
        for pair in candidates.windows(2) {
            assert!(pair[0]["value"].as_f64() >= pair[1]["value"].as_f64());
        }
        let bag = endgame_of(
            "tiny-end-first-bag",
            &[S, E, A, T, C, A, T],
            &[A, T, E, S, E, A, T, E],
            "",
        )
        .unwrap();
        assert!(
            do_endgame_step(bag, r#"{"ms":0,"first_moves_ms":1}"#, frozen, held_bytes).is_err()
        );
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
            serde_json::json!({"bag": 0, "steps": 0, "nodes": 0, "exact": false,
                "play": null, "spread": 0.0, "depth": 0, "line": []})
        );
        let answer = endgame_step(handle, r#"{"ms":60000}"#);
        assert_eq!(answer["bag"], 0);
        assert_eq!(answer["steps"], 1);
        assert!(answer["nodes"].as_u64().unwrap() > 0, "{answer}");
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
        let mut solver =
            endgame::EndgameSolver::<kwg::Node24, kwg::Node24, true, u32, false, false>::with_generator(
                &game_config,
                &kwg.kwg,
                &arm_tables,
                movegen::KurniaMoveGenerator::try_new(&game_config).unwrap(),
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
            do_endgame_step(handle, r#"{"ms":1}"#, frozen, held_bytes).unwrap_err(),
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
            let mut solver = endgame::EndgameSolver::<
                kwg::Node24,
                kwg::Node24,
                true,
                u32,
                false,
                false,
            >::with_generator(
                &game_config,
                &kwg.kwg,
                &arm_tables,
                movegen::KurniaMoveGenerator::try_new(&game_config).unwrap(),
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
        let rack = [S, B, D, F, G, H, I];
        let others = [E, S, B, D, F, G, H, I];
        let handle = endgame_of("tiny-end-bag", &rack, &others, "").unwrap();
        // read before any step: no depth searched, nothing to play, no gain,
        // and no win
        let before = endgame_step(handle, r#"{"ms":0}"#);
        assert_eq!(
            before,
            serde_json::json!({"bag": 1, "steps": 0, "nodes": 0, "exact": false,
                "play": null, "spread": 0.0, "depth": 0, "tiles": [], "candidates": [],
                "endgames": 0})
        );
        let answer = endgame_step(handle, r#"{"ms":60000}"#);
        assert_eq!(answer["bag"], 1);
        assert_eq!(answer["exact"], true);
        // the endgames it searched and their nodes, and every move of its last
        // pass valued
        assert!(answer["endgames"].as_u64().unwrap() > 0, "{answer}");
        assert!(answer["nodes"].as_u64().unwrap() > 0, "{answer}");
        let moves = answer["moves"].as_array().unwrap();
        assert!(
            moves[1].as_u64().unwrap() > 0 && moves[0] == moves[1],
            "{answer}"
        );
        assert!(answer["depth"].as_i64().unwrap() >= 1, "{answer}");
        assert!(answer["play"].is_object(), "{answer}");
        // every move the pass valued, best first, the first the answer's own,
        // each with its tiles, all exact
        let candidates = answer["candidates"].as_array().unwrap();
        assert!(!candidates.is_empty(), "{answer}");
        assert_eq!(candidates[0]["play"], answer["play"]);
        assert_eq!(candidates[0]["value"], answer["value"]);
        assert_eq!(candidates[0]["win"], answer["win"]);
        assert_eq!(candidates[0]["tiles"], answer["tiles"]);
        assert!(
            candidates
                .iter()
                .all(|c| c["exact"] == true && c["tiles"].is_array()),
            "{answer}"
        );
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

    // two tiles in the bag with estimates asked for, and no limit: the answer
    // the steps end on is the one without them, every candidate exact; with
    // fewer than two in the bag, asking is refused
    #[test]
    fn an_endgame_asked_for_estimates_ends_as_one_without() {
        let rack = [S, B, D, F, G, H, I];
        let others = [E, S, B, D, F, G, H, I, J];
        let solve = |more: &str| {
            let more = format!(r#","model":"random_draws","exchanges":[0,0]{more}"#);
            let handle = endgame_of("tiny-end-estimates", &rack, &others, &more).unwrap();
            let answer = endgame_step(handle, r#"{"ms":60000}"#);
            do_endgame_free(handle).unwrap();
            answer
        };
        let plain = solve("");
        let estimated = solve(r#","estimates":true"#);
        assert_eq!(estimated["exact"], true, "{estimated}");
        for field in ["play", "value", "win", "draws", "candidates"] {
            assert_eq!(estimated[field], plain[field], "{field}");
        }
        assert!(
            endgame_of(
                "tiny-end-estimates",
                &[S, E, A, T],
                &[A, T, E],
                r#","estimates":true"#
            )
            .is_err()
        );
    }

    // two tiles in the bag searched in two parts (a page's two workers): each
    // part says it is one, and each candidate's draws from both are the whole
    // search's; a part with fewer than two tiles in the bag is refused
    #[test]
    fn an_endgame_in_parts_draws_what_the_whole_does() {
        let rack = [S, B, D, F, G, H, I];
        let others = [E, S, B, D, F, G, H, I, J];
        let solve = |part: &str| {
            let more = format!(r#","model":"random_draws","exchanges":[0,0]{part}"#);
            let handle = endgame_of("tiny-end-parts", &rack, &others, &more).unwrap();
            let mut answer = endgame_step(handle, r#"{"ms":60000}"#);
            answer["candidates"] = serde_json::Value::Array(with_draws(handle, &answer));
            do_endgame_free(handle).unwrap();
            answer
        };
        let whole = solve("");
        assert!(whole.get("part").is_none(), "{whole}");
        let parts = [solve(r#","part":[0,2]"#), solve(r#","part":[1,2]"#)];
        assert_eq!(parts[0]["part"], serde_json::json!([0, 2]));
        assert_eq!(parts[1]["part"], serde_json::json!([1, 2]));
        let mut merged = std::collections::BTreeMap::<String, Vec<String>>::new();
        for part in &parts {
            assert_eq!(part["exact"], true, "{part}");
            for candidate in part["candidates"].as_array().unwrap() {
                let at = merged.entry(candidate["play"].to_string()).or_default();
                for draw in candidate["draws"].as_array().unwrap() {
                    at.push(draw.to_string());
                }
            }
        }
        let candidates = whole["candidates"].as_array().unwrap();
        assert_eq!(merged.len(), candidates.len(), "{whole}");
        for candidate in candidates {
            let mut want = (candidate["draws"].as_array().unwrap().iter())
                .map(|draw| draw.to_string())
                .collect::<Vec<_>>();
            let mut got = merged.remove(&candidate["play"].to_string()).unwrap();
            want.sort_unstable();
            got.sort_unstable();
            assert_eq!(got, want, "{candidate}");
        }
        assert!(
            endgame_of(
                "tiny-end-parts",
                &[S, E, A, T],
                &[A, T, E],
                r#","part":[0,2]"#
            )
            .is_err()
        );
    }

    // two tiles in the bag: the answer is wolges's own search of the position
    // (random draws; info sets is taken the same way, but its search of even
    // this position is too long for a test), each set of tiles the bag may hold
    // with its deals, and every play valued, the answer's first
    #[test]
    fn an_endgame_with_two_tiles_in_the_bag_is_wolges_s_search() {
        let rack = [S, B, D, F, G, H, I];
        let others = [E, S, B, D, F, G, H, I, J];
        let more = r#","model":"info_sets","exchanges":[0,0]"#;
        let handle = endgame_of("tiny-end-bag2", &rack, &others, more).unwrap();
        assert_eq!(endgame_step(handle, r#"{"ms":0}"#)["bag"], 2);
        do_endgame_free(handle).unwrap();
        let (name, model) = ("random_draws", endgame::PegModel::RandomDraws);
        let more = format!(r#","model":"{name}","exchanges":[0,0]"#);
        let handle = endgame_of("tiny-end-bag2", &rack, &others, &more).unwrap();
        let before = endgame_step(handle, r#"{"ms":0}"#);
        assert_eq!(
            before,
            serde_json::json!({"bag": 2, "steps": 0, "nodes": 0, "exact": false,
                    "play": null, "spread": 0.0, "depth": 0, "draws": [],
                    "candidates": [], "endgames": 0}),
            "{name}"
        );
        let answer = endgame_step(handle, r#"{"ms":60000}"#);
        assert_eq!(answer["bag"], 2, "{name}");
        assert_eq!(answer["exact"], true, "{name}");
        assert!(answer["endgames"].as_u64().unwrap() > 0, "{name}: {answer}");
        assert!(answer["nodes"].as_u64().unwrap() > 0, "{name}: {answer}");
        let plain = answer["candidates"].as_array().unwrap();
        assert!(
            plain.iter().all(|c| c.get("draws").is_none()),
            "{name}: {answer}"
        );
        let candidates = &with_draws(handle, &answer);
        let past = format!(r#"{{"ms":0,"draws_of":{}}}"#, candidates.len());
        assert!(
            do_endgame_step(handle, &past, frozen, held_bytes).is_err(),
            "{name}"
        );
        let whole = candidates[0]["draws"].as_array().unwrap();
        let page = endgame_step(handle, r#"{"ms":0,"draws_of":0,"draws_range":[1,2]}"#);
        let page = &page["candidates"][0];
        assert_eq!(page["draws"].as_array().unwrap()[..], whole[1..2], "{name}");
        assert_eq!(page["draws_total"], whole.len(), "{name}");
        let past = format!(
            r#"{{"ms":0,"draws_of":0,"draws_range":[{0},{0}]}}"#,
            whole.len() + 1
        );
        let past = endgame_step(handle, &past);
        assert_eq!(
            past["candidates"][0]["draws"],
            serde_json::json!([]),
            "{name}"
        );
        let alone = r#"{"ms":0,"draws_range":[0,1]}"#;
        assert!(do_endgame_step(handle, alone, frozen, held_bytes).is_err());
        let brief = endgame_step(handle, r#"{"ms":0,"brief":true}"#);
        let mut state = answer.clone();
        for key in ["candidates", "draws", "tiles", "line"] {
            state.as_object_mut().unwrap().remove(key);
        }
        assert_eq!(brief, state, "{name}");
        let both = r#"{"ms":0,"brief":true,"draws_of":0}"#;
        assert!(
            do_endgame_step(handle, both, frozen, held_bytes).is_err(),
            "{name}"
        );
        assert_eq!(answer["play"], candidates[0]["play"], "{name}");
        assert_eq!(answer["value"], candidates[0]["value"], "{name}");
        assert_eq!(answer["win"], candidates[0]["win"], "{name}");
        let all = candidates.len() as u64;
        assert_eq!(answer["moves"], serde_json::json!([all, all]), "{name}");
        let draws = answer["draws"].as_array().unwrap();
        assert!(
            draws
                .iter()
                .all(|d| d["tiles"].as_array().unwrap().len() == 2),
            "{name}: {answer}"
        );
        // every candidate with its own draws, the answer's the first's, each
        // with its chance to win, the chances making the candidate's win
        assert_eq!(candidates[0]["draws"], answer["draws"], "{name}");
        for candidate in candidates {
            let own = candidate["draws"].as_array().unwrap();
            assert_eq!(own.len(), draws.len(), "{name}: {candidate}");
            assert_eq!(candidate["exact"], true, "{name}: {candidate}");
            let total = own
                .iter()
                .map(|d| d["count"].as_f64().unwrap())
                .sum::<f64>();
            let won = (own.iter())
                .map(|d| {
                    let win = d["win"].as_f64().unwrap();
                    assert!((0.0..=1.0).contains(&win), "{name}: {candidate}");
                    d["count"].as_f64().unwrap() * win
                })
                .sum::<f64>();
            assert!(
                (won / total - candidate["win"].as_f64().unwrap()).abs() < 1e-9,
                "{name}: {candidate}"
            );
            assert_eq!(
                candidate["deals"].as_f64(),
                Some(total),
                "{name}: {candidate}"
            );
            // under each set, the draws the play makes from it (random draws),
            // their deals within the set, their gains, and their chances making
            // it; a pass draws nothing
            let passes = candidate["play"]["action"] == "exchange"
                && candidate["play"]["tiles"]
                    .as_array()
                    .is_some_and(Vec::is_empty);
            for row in own {
                if passes {
                    assert!(row.get("drawn").is_none(), "{name}: {row}");
                    continue;
                }
                let drawn = row["drawn"].as_array().unwrap();
                assert!(!drawn.is_empty(), "{name}: {row}");
                let deals = drawn
                    .iter()
                    .map(|d| d["count"].as_f64().unwrap())
                    .sum::<f64>();
                let gain = (drawn.iter())
                    .map(|d| d["count"].as_f64().unwrap() * d["value"].as_f64().unwrap())
                    .sum::<f64>()
                    / deals;
                let chance = (drawn.iter())
                    .map(|d| d["count"].as_f64().unwrap() * d["win"].as_f64().unwrap())
                    .sum::<f64>()
                    / deals;
                assert!(
                    (gain - row["value"].as_f64().unwrap()).abs() < 0.002,
                    "{name}: {row}"
                );
                assert!(
                    (chance - row["win"].as_f64().unwrap()).abs() < 1e-9,
                    "{name}: {row}"
                );
            }
        }

        // wolges's search on the same position
        let Tables {
            kwg, game_config, ..
        } = tables("tiny-end-bag2", "noleave", "CrosswordGame").unwrap();
        let mut kibitzer = kibitzer::Kibitzer::new();
        let board = serde_json::from_str::<Vec<Vec<i8>>>(&late_board(
            &rack.iter().chain(&others).copied().collect::<Vec<_>>(),
        ))
        .unwrap();
        kibitzer.prepare(&game_config, &rack, &board).unwrap();
        let arm_tables = anagrams::ArmTables::None;
        let mut solver =
            endgame::EndgameSolver::<kwg::Node24, kwg::Node24, true, u32, false, false>::with_generator(
                &game_config,
                &kwg.kwg,
                &arm_tables,
                movegen::KurniaMoveGenerator::try_new(&game_config).unwrap(),
            )
            .unwrap();
        let mut sorted = rack.to_vec();
        sorted.sort_unstable();
        let result = solver
            .solve_peg(
                endgame::PegBagPosition {
                    mover: 0,
                    board: kibitzer.board_tiles.clone(),
                    mover_rack: sorted,
                    unseen_tally: kibitzer.available_tally.clone(),
                    bag_size: 2,
                    score_diff: equity::Equity::ZERO,
                    exchanges: [0, 0],
                    model,
                },
                &endgame::Limits::NONE,
            )
            .unwrap()
            .unwrap();
        let play: kibitzer::JsonPlay = result.best_move.as_ref().unwrap().into();
        assert_eq!(
            answer["play"],
            serde_json::to_value(&play).unwrap(),
            "{name}"
        );
        assert_eq!(answer["win"].as_f64().unwrap(), result.win_pct, "{name}");
        assert_eq!(
            answer["value"].as_f64().unwrap(),
            points(result.expected_margin),
            "{name}"
        );
        assert_eq!(draws.len(), result.hypotheses.len(), "{name}");
        do_endgame_free(handle).unwrap();
    }

    // a clock that moves a millisecond each time it is read
    fn ticking() -> std::time::Duration {
        static READS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::time::Duration::from_millis(READS.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }

    // an empty bag's steps, each four reads of the clock, deepen on from the
    // depth the steps before finished: never a shallower one, and the steps end
    // with the endgame's own value. Steps that each began at depth 1 again
    // spent their reads walking the depths done and never got past them
    #[test]
    fn an_empty_bags_steps_deepen_on_from_the_depth_finished() {
        let (rack, others) = ([S, E, A, T, C, A, T], [A, T, E, S, E, A, T]);
        let handle = endgame_of("tiny-end-deepen", &rack, &others, "").unwrap();
        let whole = endgame_of("tiny-end-deepen", &rack, &others, "").unwrap();
        let exact = endgame_step(whole, r#"{"ms":60000}"#);
        assert_eq!(exact["exact"], true);
        let mut finished = 0;
        let mut nodes = 0;
        for step in 1.. {
            assert!(step <= 1000, "the steps did not end in a thousand");
            let answer =
                report(&do_endgame_step(handle, r#"{"ms":4}"#, ticking, held_bytes).unwrap());
            // every step searches more nodes, whether its answer is taken or not
            let now = answer["nodes"].as_u64().unwrap();
            assert!(now > nodes, "step {step}: {now} nodes after {nodes}");
            nodes = now;
            if answer["exact"] == true {
                assert_eq!(answer["value"], exact["value"]);
                assert!(step > 2, "{step} steps");
                break;
            }
            let endgames = ENDGAMES.lock().unwrap();
            let (depth, _) = endgames.live[&handle].finished.unwrap();
            assert!(
                depth >= finished,
                "step {step}: depth {depth} after {finished}"
            );
            assert_eq!(answer["depth"], depth, "step {step}");
            finished = depth;
        }
        do_endgame_free(handle).unwrap();
        do_endgame_free(whole).unwrap();
    }

    // a tile in the bag, the search stepped: each step's answer counts more
    // endgames and nodes, and says how many moves the pass under way has valued,
    // of all, some step stopping partway through a pass; a last step with no
    // time limit ends the search, every move of its pass valued
    #[test]
    fn a_tile_in_the_bags_steps_count_their_endgames_and_nodes() {
        let rack = [S, B, D, F, G, H, I];
        let others = [E, S, B, D, F, G, H, I];
        let handle = endgame_of("tiny-end-bag-steps", &rack, &others, "").unwrap();
        let (mut endgames, mut nodes, mut partway) = (0, 0, false);
        let mut counted = |answer: &serde_json::Value| {
            let (now_endgames, now_nodes) = (
                answer["endgames"].as_u64().unwrap(),
                answer["nodes"].as_u64().unwrap(),
            );
            assert!(now_endgames > endgames, "{answer}");
            assert!(now_nodes > nodes, "{answer}");
            (endgames, nodes) = (now_endgames, now_nodes);
            let moves = answer["moves"].as_array().unwrap();
            let (valued, of) = (moves[0].as_u64().unwrap(), moves[1].as_u64().unwrap());
            assert!(valued <= of && of > 0, "{answer}");
            partway |= valued < of;
            (valued, of)
        };
        for _ in 0..50 {
            let answer =
                report(&do_endgame_step(handle, r#"{"ms":4}"#, ticking, held_bytes).unwrap());
            assert_eq!(answer["exact"], false, "{answer}");
            counted(&answer);
        }
        let answer = endgame_step(handle, r#"{"ms":60000}"#);
        assert_eq!(answer["exact"], true, "{answer}");
        let (valued, of) = counted(&answer);
        assert_eq!(valued, of);
        assert!(partway, "no step stopped partway through a pass");
        do_endgame_free(handle).unwrap();
    }

    // an empty bag's answer gives way to a step's only when that is exact,
    // deeper, or at its depth worth more, so a step that stopped sooner than
    // one before it leaves the deeper answer
    #[test]
    fn an_empty_bags_answer_gives_way_only_to_a_better_one() {
        let answer = |exact: bool, depth: i8, value: f64| EndgameAnswer {
            part: None,
            out_of_memory: false,
            bag: 0,
            steps: 1,
            nodes: 0,
            exact,
            play: Some(kibitzer::JsonPlay::Exchange {
                tiles: Vec::new().into(),
            }),
            value: Some(value),
            spread: value,
            depth: Some(depth),
            line: Some(Vec::new()),
            win: None,
            tiles: None,
            draws: None,
            candidates: None,
            endgames: None,
            moves: None,
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
        none.value = None;
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
                nodes: 99,
                ..answer(false, 4, 30.0)
            },
            false,
        );
        assert_eq!(
            (steps.depth, steps.value, steps.steps, steps.nodes),
            (Some(5), Some(19.0), 7, 99)
        );
        // with a tile in the bag every step's answer is taken, its depth never
        // below the one before
        let mut in_bag = answer(false, 2, 1.0);
        in_bag.take(answer(false, 1, 2.0), true);
        assert_eq!((in_bag.depth, in_bag.value), (Some(2), Some(2.0)));
    }

    // a request that gives no memory caps none, the table growing until the
    // page refuses it more; one that gives MiB caps that many
    #[test]
    fn an_endgame_caps_its_memory_only_when_asked() {
        for (more, cap) in [
            ("", None),
            (r#","max_memory_mb":300"#, Some(300 << 20)),
            (
                r#","max_memory_mb":4096"#,
                Some(4096usize.saturating_mul(1 << 20)),
            ),
            (
                &format!(r#","max_memory_mb":{}"#, usize::MAX)[..],
                Some(usize::MAX),
            ),
        ] {
            let handle = endgame_of("tiny-end-cap", &[S], &[A, T], more).unwrap();
            let max_memory = ENDGAMES.lock().unwrap().live[&handle].max_memory;
            assert_eq!(max_memory, cap, "{more}");
            do_endgame_free(handle).unwrap();
        }
    }

    // a step says it is out of memory exactly when it stops short of exact
    // holding its cap (1 MiB, less than a buffer kept while it steps holds,
    // whatever other tests hold), in an empty bag's search and in a
    // pre-endgame's; one with no cap never says so
    #[test]
    fn an_endgame_step_says_when_it_is_out_of_memory() {
        let cases = [
            (&[S, E, A, T][..], &[A, T, E][..], ""),
            (
                &[S, B, D, F, G, H, I][..],
                &[E, S, B, D, F, G, H, I, J][..],
                r#","model":"random_draws","exchanges":[0,0]"#,
            ),
        ];
        let kept = std::hint::black_box(vec![1u8; 2 << 20]);
        for (rack, others, model) in cases {
            for (cap, out) in [("", false), (r#","max_memory_mb":1"#, true)] {
                let more = format!("{model}{cap}");
                let handle = endgame_of("tiny-end-oom", rack, others, &more).unwrap();
                let answer = endgame_step(handle, r#"{"ms":60000}"#);
                let short = answer["exact"] == false;
                let says = answer.get("out_of_memory").is_some();
                assert_eq!(says, out && short, "{more}: {answer}");
                if says {
                    assert_eq!(answer["out_of_memory"], true, "{more}: {answer}");
                }
                do_endgame_free(handle).unwrap();
            }
        }
        drop(kept);
    }

    // a what-if warmed by an endgame before it answers as a cold one does, on
    // fewer nodes, taking that one's table; refused with tiles in the bag, on a
    // board lacking a tile of that one's, or from no such endgame
    #[test]
    fn a_warm_what_if_answers_as_a_cold_one() {
        let (rack, others) = ([S, E, A, T], [A, T, E]);
        let lexicon = "tiny-end-warm";
        let cold = endgame_of(lexicon, &rack, &others, "").unwrap();
        let cold_answer = endgame_step(cold, r#"{"ms":60000}"#);
        assert_eq!(cold_answer["exact"], true);
        let source = endgame_of(lexicon, &rack, &others, "").unwrap();
        assert_eq!(endgame_step(source, r#"{"ms":60000}"#), cold_answer);
        let warm = endgame_of(
            lexicon,
            &rack,
            &others,
            &format!(r#","warm_from":{source}"#),
        )
        .unwrap();
        let warm_answer = endgame_step(warm, r#"{"ms":60000}"#);
        assert_eq!(warm_answer["exact"], true);
        for field in ["play", "value", "spread", "line"] {
            assert_eq!(warm_answer[field], cold_answer[field], "{field}");
        }
        assert!(
            warm_answer["nodes"].as_u64().unwrap() < cold_answer["nodes"].as_u64().unwrap(),
            "{warm_answer} {cold_answer}"
        );
        // the source keeps its answer; its table left it
        assert_eq!(endgame_step(source, r#"{"ms":0}"#), cold_answer);

        // a board lacking a tile of the source's
        let late = serde_json::from_str::<Vec<Vec<i8>>>(&late_board(
            &rack.iter().chain(&others).copied().collect::<Vec<_>>(),
        ))
        .unwrap();
        let mut lacking = late.clone();
        lacking[2][8] = 0;
        let err = do_endgame_new(&format!(
            r#"{{"rack":[{S},{E},{A},{T},{T}],"board":{},"lexicon":"{lexicon}","rules":"CrosswordGame","scores":[0,0],"turn":0,"zero_turns":0,"pass_turns":0,"warm_from":{warm}}}"#,
            serde_json::to_string(&lacking).unwrap()
        ))
        .unwrap_err();
        assert!(err.contains("lacks a tile"), "{err}");
        // tiles in the bag
        let bag = endgame_of(
            lexicon,
            &[S, E, A, T, C, A, T],
            &[A, T, E, S, E, A, T, E],
            "",
        )
        .unwrap();
        let err =
            endgame_of(lexicon, &rack, &others, &format!(r#","warm_from":{bag}"#)).unwrap_err();
        assert!(err.contains("empty bag"), "{err}");
        // no such endgame
        do_endgame_free(cold).unwrap();
        let err =
            endgame_of(lexicon, &rack, &others, &format!(r#","warm_from":{cold}"#)).unwrap_err();
        assert!(err.contains("freed"), "{err}");
        for handle in [source, warm, bag] {
            do_endgame_free(handle).unwrap();
        }
    }

    // what endgame_new and endgame_step refuse, each before it solves
    #[test]
    fn an_endgame_refuses_what_it_cannot_solve() {
        let full = [S, E, A, T, C, A, T];
        for (rack, others, more, wanted) in [
            (
                &full[..],
                &[A, T, E, S, E, A, T, E, E][..],
                r#","exchanges":[0,0]"#,
                "model: \"info_sets\" or \"random_draws\" with 2 tiles in the bag, found None",
            ),
            (
                &full[..],
                &[A, T, E, S, E, A, T, E, E][..],
                r#","model":"blind","exchanges":[0,0]"#,
                "model: \"info_sets\" or \"random_draws\" with 2 tiles in the bag, found Some(\"blind\")",
            ),
            (
                &full[..],
                &[A, T, E, S, E, A, T, E, E][..],
                r#","model":"info_sets""#,
                "exchanges: each seat's, with 2 tiles in the bag",
            ),
            (
                &full[..],
                &[A, T, E, S, E, A, T, E, E][..],
                r#","model":"info_sets","exchanges":[0,-1]"#,
                "exchanges: each must be >= 0, found [0, -1]",
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
                "max_memory_mb: at least 1, found 0",
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
            do_endgame_step(handle, "{}", frozen, held_bytes).unwrap_err(),
            "endgame_step: give ms"
        );
        assert_eq!(
            do_endgame_step(handle, r#"{"ms":-1}"#, frozen, held_bytes).unwrap_err(),
            "ms: -1 is not a time to run"
        );
        do_endgame_free(handle).unwrap();
    }

    // more english tiles, none of them in a tiny lexicon's words
    const B: u8 = 2;
    const D: u8 = 4;
    const F: u8 = 6;
    const G: u8 = 7;
    const H: u8 = 8;
    const I: u8 = 9;
    const J: u8 = 10;

    // a board as json for threats: CAT across row 11 from column 6. Every other
    // tile not unseen and not on the watcher's rack lies in a block from the
    // top left row by row. Its first row and first column hold tiles no word
    // has. So no play touches it.
    fn threats_board(unseen: &[u8], rack: &[u8]) -> String {
        do_this_on_startup().unwrap();
        let rules = CACHED_GAME_CONFIG.read().unwrap();
        let alphabet = rules["CrosswordGame"].alphabet();
        let mut left = (0..alphabet.len())
            .map(|tile| alphabet.freq(tile))
            .collect::<Vec<_>>();
        for &tile in unseen.iter().chain(rack).chain(&[C, A, T]) {
            left[tile as usize] -= 1;
        }
        assert_eq!(left[0], 0, "a blank goes unseen or on the rack");
        let (mut junk, mut others) = (Vec::new(), Vec::new());
        for tile in 1..alphabet.len() {
            for _ in 0..left[tile as usize] {
                if [A, C, E, S, T].contains(&tile) {
                    others.push(tile);
                } else {
                    junk.push(tile);
                }
            }
        }
        let mut board = vec![vec![0i8; 15]; 15];
        let block = junk.len() + others.len();
        assert!(block / 15 < 10, "the block stays clear of CAT");
        for at in 0..block {
            let (row, col) = (at / 15, at % 15);
            board[row][col] = if row == 0 || col == 0 {
                junk.pop()
            } else {
                others.pop().or_else(|| junk.pop())
            }
            .unwrap() as i8;
        }
        board[11][6] = C as i8;
        board[11][7] = A as i8;
        board[11][8] = T as i8;
        serde_json::to_string(&board).unwrap()
    }

    // a threats answer to a request on threats_board, parsed
    fn threats_of(unseen: &[u8], rack: &[u8], more: &str) -> serde_json::Value {
        tiny_lexicon("tiny-threats");
        let req = format!(
            r#"{{"rack":{},"board":{},"lexicon":"tiny-threats","rules":"CrosswordGame"{more}}}"#,
            serde_json::to_string(rack).unwrap(),
            threats_board(unseen, rack),
        );
        serde_json::from_str(&do_threats(&req, || 0.0).unwrap()).unwrap()
    }

    fn assert_near(found: &serde_json::Value, want: f64) {
        let found = found.as_f64().unwrap();
        assert!((found - want).abs() < 1e-12, "{found} is not {want}");
    }

    // each square some play places a tile on, as (row, col, square), row by
    // row; every square a tile is on is null, every other one not
    fn threat_squares(answer: &serde_json::Value) -> Vec<(i64, i64, serde_json::Value)> {
        let rows = answer["squares"].as_array().unwrap();
        assert_eq!(rows.len(), 15);
        let mut reached = Vec::new();
        for (row, squares) in (0..).zip(rows) {
            let squares = squares.as_array().unwrap();
            assert_eq!(squares.len(), 15);
            for (col, square) in (0..).zip(squares) {
                if square.is_null() {
                    continue;
                }
                if square["best"] == 0 {
                    assert_eq!(square["chance"], 0.0);
                    assert_eq!(square["expected"], 0.0);
                    assert_eq!(square["reach"], 0.0);
                } else {
                    // a rack holding the best play there holds a play there
                    let (chance, reach) = (square["chance"].as_f64(), square["reach"].as_f64());
                    assert!(reach.unwrap() >= chance.unwrap() - 1e-12, "{square}");
                    reached.push((row, col, square.clone()));
                }
            }
        }
        reached
    }

    // each play, as (down, lane, idx, word, tiles)
    fn threat_plays(answer: &serde_json::Value) -> Vec<serde_json::Value> {
        (answer["plays"].as_array().unwrap().iter())
            .map(|play| {
                serde_json::json!([
                    play["down"],
                    play["lane"],
                    play["idx"],
                    play["word"],
                    play["tiles"]
                ])
            })
            .collect()
    }

    // unseen: two S and seven tiles no word has. A rack of 7 of them is one of
    // C(9, 7) = 36 ways. There are 29 racks: 1 with no S, 7 with one S, and 21
    // with two. Only the one with no S (all seven others) holds no play: each
    // play places one S (cats, scat, as down). So each play, each square some
    // play places a tile on, and the best play are held 35 times in 36.
    #[test]
    fn threats_deal_the_opponent_a_rack_from_the_tiles_unseen() {
        let unseen = [S, S, B, D, F, G, H, I, J];
        let rack = [0, 0, I, I, I, I, I];
        let all = threats_of(&unseen, &rack, "");
        assert_eq!(all["racks"], 29);
        assert_eq!(all["count"], 3);
        let mut plays = threat_plays(&all);
        plays.sort_by_key(|play| play.to_string());
        let mut want = vec![
            serde_json::json!([false, 11, 6, [0, 0, 0, S], [S]]),
            serde_json::json!([false, 11, 5, [S, 0, 0, 0], [S]]),
            serde_json::json!([true, 7, 11, [0, S], [S]]),
        ];
        want.sort_by_key(|play| play.to_string());
        assert_eq!(plays, want);
        let listed = all["plays"].as_array().unwrap();
        for play in listed {
            assert_near(&play["chance"], 35.0 / 36.0);
        }
        let top = listed[0]["score"].as_i64().unwrap();
        assert!(
            listed
                .iter()
                .all(|play| play["score"].as_i64().unwrap() <= top)
        );
        assert_near(&all["expected_best"], 35.0 / 36.0 * top as f64);
        assert_eq!(all["bingo"], 0.0);
        assert_eq!(all["exact"], true);
        assert_eq!(all["covered"], 1.0);
        assert_eq!(all["samples"], 0);
        assert!(all["squares"][11][6].is_null());
        let squares = threat_squares(&all);
        assert_eq!(squares.len(), 3);
        for (row, col, square) in &squares {
            assert_near(&square["chance"], 35.0 / 36.0);
            let best = square["best"].as_f64().unwrap();
            assert_near(&square["expected"], 35.0 / 36.0 * best);
            assert_near(&square["reach"], 35.0 / 36.0);
            let (row, col) = (Some(*row), Some(*col));
            let placing = listed.iter().find(|play| {
                let (lane, idx) = (play["lane"].as_i64(), play["idx"].as_i64());
                let word = play["word"].as_array().unwrap();
                let at = word.iter().position(|tile| tile != 0).unwrap() as i64;
                if play["down"] == true {
                    (lane, idx.map(|idx| idx + at)) == (col, row)
                } else {
                    (lane, idx.map(|idx| idx + at)) == (row, col)
                }
            });
            assert_eq!(square["best"], placing.unwrap()["score"]);
        }
        // the best two of them, the best first
        let two = threats_of(&unseen, &rack, r#","count":2"#);
        assert_eq!(two["count"], 3);
        assert_eq!(two["plays"].as_array().unwrap()[..], listed[..2]);
    }

    // the opponent kept an S or a B, alike. With the S it holds every play.
    // With the B its other six are 6 of the 8 left (two S among them): none is
    // an S once in C(8, 6) = 28 ways. So each play is held 1/2 + 1/2 * 27/28 =
    // 55/56 of the time. There are 29 racks: 28 with the S kept, and the one
    // with the B but no S.
    #[test]
    fn threats_weigh_the_leaves_the_opponent_may_have_kept() {
        let kept =
            format!(r#","kept":[{{"tiles":[{S}],"weight":1}},{{"tiles":[{B}],"weight":1}}]"#);
        let answer = threats_of(&[S, S, B, D, F, G, H, I, J], &[0, 0, I, I, I, I, I], &kept);
        assert_eq!(answer["racks"], 29);
        for play in answer["plays"].as_array().unwrap() {
            assert_near(&play["chance"], 55.0 / 56.0);
        }
        for (_, _, square) in threat_squares(&answer) {
            assert_near(&square["chance"], 55.0 / 56.0);
        }
        // the same leaves as leaves
        let leaves = kept.replace("kept", "leaves");
        let again = threats_of(
            &[S, S, B, D, F, G, H, I, J],
            &[0, 0, I, I, I, I, I],
            &leaves,
        );
        assert_eq!(again, answer);
    }

    // fewer tiles unseen than a rack holds: the opponent holds them all, so
    // every play of them is certain, a blank's as any letter as well as the S
    // as itself (cats with the S, or with the blank as an S)
    #[test]
    fn threats_give_the_opponent_every_tile_unseen_when_too_few_are_left() {
        let answer = threats_of(&[0, S, B], &[0, I, I, I, I, I, I], "");
        assert_eq!(answer["racks"], 1);
        let listed = answer["plays"].as_array().unwrap();
        assert_eq!(answer["count"], listed.len());
        for play in listed {
            assert_eq!(play["chance"], 1.0);
        }
        for (_, _, square) in threat_squares(&answer) {
            assert_eq!(square["chance"], 1.0);
            assert_eq!(square["expected"], square["best"].as_f64().unwrap());
        }
        assert_eq!(
            answer["expected_best"],
            listed[0]["score"].as_f64().unwrap()
        );
        assert!(answer["bingo"].is_null(), "three tiles, not a rack's");
        let plays = threat_plays(&answer);
        let blank_s = -(S as i8);
        for want in [
            serde_json::json!([false, 11, 6, [0, 0, 0, S], [S]]),
            serde_json::json!([false, 11, 6, [0, 0, 0, blank_s], [0]]),
        ] {
            assert!(plays.contains(&want), "{want}");
        }
    }

    // the tiles unseen as counts, the rack then empty: the same answer as the
    // rack gives. Not both.
    #[test]
    fn threats_take_the_tiles_unseen_as_counts() {
        let unseen = [S, S, B, D, F, G, H, I, J];
        let rack = [0, 0, I, I, I, I, I];
        let want = threats_of(&unseen, &rack, "");
        let mut counts = vec![0u8; 27];
        for tile in unseen {
            counts[tile as usize] += 1;
        }
        let board = threats_board(&unseen, &rack);
        let req = |rack: &str| {
            format!(
                r#"{{"rack":{rack},"board":{board},"lexicon":"tiny-threats","rules":"CrosswordGame","unseen":{}}}"#,
                serde_json::to_string(&counts).unwrap(),
            )
        };
        let found = do_threats(&req("[]"), || 0.0).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&found).unwrap(),
            want
        );
        assert_eq!(
            do_threats(&req("[9]"), || 0.0).unwrap_err(),
            "rack, unseen: give one or the other"
        );
    }

    // an opponent holding six tiles, 6 of the 9 unseen: none is an S in C(7, 6)
    // = 7 of C(9, 6) = 84 ways, so each play is held 11 times in 12, and no
    // rack's tiles are there to bingo with. More than a rack is an error.
    #[test]
    fn threats_deal_the_opponent_the_tiles_it_holds() {
        let unseen = [S, S, B, D, F, G, H, I, J];
        let rack = [0, 0, I, I, I, I, I];
        let six = threats_of(&unseen, &rack, r#","opp_rack_size":6"#);
        for play in six["plays"].as_array().unwrap() {
            assert_near(&play["chance"], 11.0 / 12.0);
        }
        assert!(six["bingo"].is_null());
        tiny_lexicon("tiny-threats");
        let req = format!(
            r#"{{"rack":{},"board":{},"lexicon":"tiny-threats","rules":"CrosswordGame","opp_rack_size":8}}"#,
            serde_json::to_string(&rack).unwrap(),
            threats_board(&unseen, &rack),
        );
        assert_eq!(
            do_threats(&req, || 0.0).unwrap_err(),
            "opp_rack_size: max 7 tiles, found 8 tiles"
        );
    }

    // ms run out (a clock moving a whole ms each read): the half of it left
    // after the listing is gone once 16 racks are counted, the likeliest first:
    // the 7 with one S (2 ways in 36 each), then of those dealt 1 way, in tile
    // order, the one with no S and 8 with two. They hold 23/36 of the chance
    // and the plays 22/36 of it. Then 16 racks are drawn from the 13 left, each
    // with two S, so each play's chance is estimated at 22/36 plus the 13/36
    // left: 35/36, as an unbiased estimate must be here. The same seed draws
    // the same racks.
    #[test]
    fn threats_count_the_likeliest_racks_then_draw_from_the_rest() {
        let unseen = [S, S, B, D, F, G, H, I, J];
        let rack = [0, 0, I, I, I, I, I];
        tiny_lexicon("tiny-threats");
        let req = format!(
            r#"{{"rack":{},"board":{},"lexicon":"tiny-threats","rules":"CrosswordGame","ms":0,"seed":7}}"#,
            serde_json::to_string(&rack).unwrap(),
            threats_board(&unseen, &rack),
        );
        let ask = || {
            let clock = std::cell::Cell::new(0.0);
            let now = || {
                let time = clock.get();
                clock.set(time + 1.0);
                time
            };
            let answer = do_threats(&req, now).unwrap();
            assert_eq!(
                clock.get(),
                5.0,
                "read when asked, listing, listed, cut, drawn"
            );
            serde_json::from_str::<serde_json::Value>(&answer).unwrap()
        };
        let answer = ask();
        assert_eq!(answer["exact"], false);
        assert_eq!(answer["list_ms"], 1.0);
        assert_near(&answer["covered"], 23.0 / 36.0);
        assert_eq!(answer["samples"], 16);
        assert_eq!(answer["racks"], 29);
        for play in answer["plays"].as_array().unwrap() {
            assert_near(&play["chance"], 35.0 / 36.0);
        }
        for (_, _, square) in threat_squares(&answer) {
            assert_near(&square["chance"], 35.0 / 36.0);
        }
        assert_eq!(ask(), answer);
    }

    // the opponent known to hold an S: every rack holds one, so every play is
    // certain
    #[test]
    fn threats_deal_around_the_tiles_the_opponent_is_known_to_hold() {
        let unseen = [S, S, B, D, F, G, H, I, J];
        let rack = [0, 0, I, I, I, I, I];
        let answer = threats_of(&unseen, &rack, &format!(r#","known":[{S}]"#));
        assert_eq!(answer["racks"], 28);
        for play in answer["plays"].as_array().unwrap() {
            assert_near(&play["chance"], 1.0);
        }
    }

    // threats a step at a time: a step whose time is up at its first read of
    // the clock (each read moves it on) counts 16 racks, the likeliest, and
    // draws 16 from the rest, as threats with ms does; a step with no ms counts
    // the rest, the answer then exact and as threats gives it. Freed once only,
    // and no step after.
    #[test]
    fn threats_step_count_every_rack_in_the_end() {
        let unseen = [S, S, B, D, F, G, H, I, J];
        let rack = [0, 0, I, I, I, I, I];
        tiny_lexicon("tiny-threats");
        let req = format!(
            r#"{{"rack":{},"board":{},"lexicon":"tiny-threats","rules":"CrosswordGame","seed":7}}"#,
            serde_json::to_string(&rack).unwrap(),
            threats_board(&unseen, &rack),
        );
        let clock = std::cell::Cell::new(0.0);
        let now = || {
            let time = clock.get();
            clock.set(time + 1.0);
            time
        };
        let handle = do_threats_new(&req, now).unwrap();
        let step = |ms: &str| {
            let answer = do_threats_step(handle, &format!(r#"{{{ms}}}"#), now).unwrap();
            serde_json::from_str::<serde_json::Value>(&answer).unwrap()
        };
        let first = step(r#""ms":1"#);
        assert_eq!(first["exact"], false);
        assert_near(&first["covered"], 23.0 / 36.0);
        assert_eq!(first["samples"], 16);
        for play in first["plays"].as_array().unwrap() {
            assert_near(&play["chance"], 35.0 / 36.0);
        }
        let mut last = step("");
        assert_eq!(last["exact"], true);
        assert_eq!(last["covered"], 1.0);
        assert_eq!(last["samples"], 0);
        let mut all = threats_of(&unseen, &rack, "");
        for answer in [&mut last, &mut all] {
            answer.as_object_mut().unwrap().remove("list_ms");
            for play in answer["plays"].as_array_mut().unwrap() {
                let chance = play["chance"].as_f64().unwrap();
                play["chance"] = serde_json::json!((chance * 1e9).round());
            }
        }
        assert_eq!(last, all);
        assert_eq!(do_threats_free(handle), Ok(()));
        let gone = format!("threats {handle}: none such, or freed");
        assert_eq!(do_threats_free(handle), Err(gone.clone()));
        assert_eq!(do_threats_step(handle, "{}", now), Err(gone));
    }

    // a step of no time weighs nothing: before any weighing every chance is 0.
    // Once every rack is weighed, a step of no time lists the plays through a
    // square (S on (11, 9) is cats's alone), as many as asked; a square off the
    // board is refused
    #[test]
    fn threats_step_list_the_plays_through_a_square() {
        let unseen = [S, S, B, D, F, G, H, I, J];
        let rack = [0, 0, I, I, I, I, I];
        tiny_lexicon("tiny-threats");
        let req = format!(
            r#"{{"rack":{},"board":{},"lexicon":"tiny-threats","rules":"CrosswordGame"}}"#,
            serde_json::to_string(&rack).unwrap(),
            threats_board(&unseen, &rack),
        );
        let handle = do_threats_new(&req, || 0.0).unwrap();
        let step = |more: &str| {
            let answer = do_threats_step(handle, &format!("{{{more}}}"), || 0.0).unwrap();
            serde_json::from_str::<serde_json::Value>(&answer).unwrap()
        };
        let none = step(r#""ms":0"#);
        assert_eq!(none["exact"], false);
        assert_eq!(none["covered"], 0.0);
        assert_eq!(none["samples"], 0);
        let all = step("");
        assert_eq!(all["exact"], true);
        let plays = all["plays"].as_array().unwrap();
        assert_eq!(plays.len(), 3);
        let through = step(r#""ms":0,"square":[11,9]"#);
        assert_eq!(through["exact"], true);
        assert_eq!(through["squares"], all["squares"]);
        let listed = through["plays"].as_array().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["word"], serde_json::json!([0, 0, 0, S]));
        let two = step(r#""ms":0,"count":2"#);
        assert_eq!(two["plays"].as_array().unwrap()[..], plays[..2]);
        assert!(
            step(r#""ms":0,"square":[0,0]"#)["plays"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            do_threats_step(handle, r#"{"ms":0,"square":[15,0]}"#, || 0.0).unwrap_err(),
            "square: [15, 0] is off the board"
        );
        do_threats_free(handle).unwrap();
    }
}
