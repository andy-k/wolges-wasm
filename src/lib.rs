// Copyright (C) 2020-2021 Andy Kurnia.

use wasm_bindgen::prelude::*;

macro_rules! mod_many {
  ($($mod: ident)+) => {
    $(#[allow(dead_code)] mod $mod;)+
  };
}
mod_many!(alphabet bites board_layout game_config klv kwg matrix movegen);

macro_rules! return_error {
    ($error:expr) => {
        return Err($error.into());
    };
}

macro_rules! console_log {
    ($($t:tt)*) => (web_sys::console::log_1(&format_args!($($t)*).to_string().into()))
}

#[wasm_bindgen(start)]
pub fn do_this_on_startup() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    CACHED_GAME_CONFIG.write().unwrap().insert(
        "CrosswordGame".into(),
        game_config::make_common_english_game_config().into(),
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
struct Question {
    rack: Vec<u8>,
    #[serde(rename = "board")]
    board_tiles: Vec<Vec<i8>>,
    #[serde(rename = "count")]
    max_gen: usize,
    lexicon: String,
    leave: String,
    rules: String,
}

// note: only this representation uses -1i8 for blank-as-A (in "board" input
// and "word" response for "action":"play"). everywhere else, use 0x81u8.

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(tag = "action")]
enum JsonPlay {
    #[serde(rename = "exchange")]
    Exchange { tiles: Box<[u8]> },
    #[serde(rename = "play")]
    Play {
        down: bool,
        lane: i8,
        idx: i8,
        word: Box<[i8]>,
        score: i16,
    },
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct JsonPlayWithEquity {
    equity: f32,
    #[serde(flatten)]
    play: JsonPlay,
}

type WasmCache<T> = std::sync::RwLock<std::collections::HashMap<String, std::sync::Arc<T>>>;

lazy_static::lazy_static! {
    static ref CACHED_KWG: WasmCache<kwg::Kwg> = Default::default();
    static ref CACHED_KLV: WasmCache<klv::Klv> = Default::default();
    static ref CACHED_GAME_CONFIG: WasmCache<game_config::GameConfig<'static>> = Default::default();
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
pub fn analyze(question_str: &str) -> Result<JsValue, JsValue> {
    let question = serde_json::from_str::<Question>(question_str).map_err(err_to_str)?;

    use_wasm_cache!(kwg, CACHED_KWG, &question.lexicon);
    use_wasm_cache!(klv, CACHED_KLV, &question.leave);
    use_wasm_cache!(game_config, CACHED_GAME_CONFIG, &question.rules);

    let alphabet = game_config.alphabet();
    let alphabet_len_without_blank = alphabet.len() - 1;

    // note: this allocates
    let mut available_tally = (0..alphabet.len())
        .map(|tile| alphabet.freq(tile))
        .collect::<Box<_>>();

    for &tile in &question.rack {
        if tile > alphabet_len_without_blank {
            return_error!(format!(
                "rack has invalid tile {}, alphabet size is {}",
                tile, alphabet_len_without_blank
            ));
        }
        if available_tally[tile as usize] > 0 {
            available_tally[tile as usize] -= 1;
        } else {
            return_error!(format!(
                "too many tile {} (bag contains only {})",
                tile,
                alphabet.freq(tile),
            ));
        }
    }

    let expected_dim = game_config.board_layout().dim();
    if question.board_tiles.len() != expected_dim.rows as usize {
        return_error!(format!(
            "board: need {} rows, found {} rows",
            expected_dim.rows,
            question.board_tiles.len()
        ));
    }
    for (row_num, row) in (0..).zip(question.board_tiles.iter()) {
        if row.len() != expected_dim.cols as usize {
            return_error!(format!(
                "board row {} (0-based): need {} cols, found {} cols",
                row_num,
                expected_dim.cols,
                row.len()
            ));
        }
    }
    let mut board_tiles =
        Vec::with_capacity((expected_dim.rows as usize) * (expected_dim.cols as usize));
    for (row_num, row) in (0..).zip(question.board_tiles.iter()) {
        for (col_num, &signed_tile) in (0..).zip(row) {
            if signed_tile == 0 {
                board_tiles.push(0);
            } else if signed_tile as u8 <= alphabet_len_without_blank {
                let tile = signed_tile as u8;
                board_tiles.push(tile);
                if available_tally[tile as usize] > 0 {
                    available_tally[tile as usize] -= 1;
                } else {
                    return_error!(format!(
                        "too many tile {} (bag contains only {})",
                        tile,
                        alphabet.freq(tile),
                    ));
                }
            } else if (!signed_tile as u8) < alphabet_len_without_blank {
                // turn -1i8, -2i8 into 0x81u8, 0x82u8
                board_tiles.push(0x81 + !signed_tile as u8);
                // verify usage of blank tile
                if available_tally[0] > 0 {
                    available_tally[0] -= 1;
                } else {
                    return_error!(format!(
                        "too many tile {} (bag contains only {})",
                        0,
                        alphabet.freq(0),
                    ));
                }
            } else {
                return_error!(format!(
                    "board row {} col {} (0-based): invalid tile {}, alphabet size is {}",
                    row_num, col_num, signed_tile, alphabet_len_without_blank
                ));
            }
        }
    }

    let mut move_generator = movegen::KurniaMoveGenerator::new(&game_config);

    let board_snapshot = &movegen::BoardSnapshot {
        board_tiles: &board_tiles,
        game_config: &game_config,
        kwg: &kwg,
        klv: &klv,
    };

    move_generator.gen_moves_unfiltered(board_snapshot, &question.rack, question.max_gen);
    let plays = &move_generator.plays;
    if false {
        console_log!("found {} moves", plays.len());
        for play in plays.iter() {
            console_log!("{} {}", play.equity, play.play.fmt(board_snapshot));
        }
    }

    let mut result = Vec::with_capacity(plays.len());
    for play in plays.iter() {
        match &play.play {
            movegen::Play::Exchange { tiles } => {
                // tiles: array of numbers. 0 for blank, 1 for A.
                result.push(JsonPlayWithEquity {
                    equity: play.equity,
                    play: JsonPlay::Exchange {
                        tiles: tiles[..].into(),
                    },
                });
            }
            movegen::Play::Place {
                down,
                lane,
                idx,
                word,
                score,
            } => {
                // turn 0x81u8, 0x82u8 into -1i8, -2i8
                let word_played = word
                    .iter()
                    .map(|&x| {
                        if x & 0x80 != 0 {
                            -((x & !0x80) as i8)
                        } else {
                            x as i8
                        }
                    })
                    .collect::<Vec<i8>>();
                // across plays: down=false, lane=row, idx=col (0-based).
                // down plays: down=true, lane=col, idx=row (0-based).
                // word: 0 for play-through, 1 for A, -1 for blank-as-A.
                result.push(JsonPlayWithEquity {
                    equity: play.equity,
                    play: JsonPlay::Play {
                        down: *down,
                        lane: *lane,
                        idx: *idx,
                        word: word_played.into(),
                        score: *score,
                    },
                });
            }
        }
    }

    Ok(serde_json::to_string(&result).map_err(err_to_str)?.into())
}
