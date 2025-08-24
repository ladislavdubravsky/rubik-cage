use crate::{
    app::{
        agent::{EvaluationTask, EvaluationTaskSpec},
        hovered_move::use_hovered_move,
        utils::apply_move_callback,
    },
    core::game::{GameState, Player},
    search::naive::{Evaluation, SearchMode},
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use yew::{platform::spawn_local, prelude::*, use_effect_with};
use yew_agent::oneshot::use_oneshot_runner;

#[derive(Properties, PartialEq)]
pub struct PlayerPanelProps {
    pub player: Player,
    pub game_state: UseStateHandle<GameState>,
    pub history: UseStateHandle<Vec<GameState>>,
    pub eval: Rc<RefCell<HashMap<u64, Evaluation>>>,
}

fn eval_to_string(eval: Option<&Evaluation>, player_id: u8) -> String {
    if eval.is_none() {
        return "Calculating...".to_string();
    }
    let Evaluation {
        score,
        moves_to_wl: moves_to_win,
    } = eval.unwrap();
    match (score, player_id) {
        (1, 0) => format!("Win in {}", moves_to_win),
        (1, 1) => format!("Loss in {}", moves_to_win),
        (-1, 0) => format!("Loss in {}", moves_to_win),
        (-1, 1) => format!("Win in {}", moves_to_win),
        (0, _) => "Draw".to_string(),
        ev => format!("Unexpected evaluation: {:?}", ev),
    }
}

#[function_component(PlayerPanel)]
pub fn player_panel(props: &PlayerPanelProps) -> Html {
    let is_turn = props.game_state.player_to_move.id == props.player.id;
    let move_list_visible = use_state(|| false);
    let eval = props.eval.clone();

    let is_won = props.game_state.won().is_some();
    let apply_move = apply_move_callback(props.game_state.clone(), props.history.clone(), is_won);

    let cubies = (0..props.game_state.remaining_cubies[props.player.id as usize]).map(|i| {
        html! {
            <div class={classes!("cubie-icon", props.player.color.to_string())} key={i} />
        }
    });

    let moves = if props.game_state.won().is_some() {
        Vec::new() // Don't show further moves if game is finished
    } else {
        props.game_state.legal_moves()
    };
    let (hovered_move, set_hovered_move) = use_hovered_move();

    // The web worker evaluating missing (non-preloaded) game state evaluations runs with pruning.
    // That avoids wasting time and calculating lots of positions we'll never need to see.
    // But it also means we'll need to call it repeatedly for new unevaluated positions.
    // We avoid the need to sync multiple workers by only allowing one to run at a time.
    let agent_running = use_state(|| false);
    let eval_task = use_oneshot_runner::<EvaluationTask>();
    let game_state = props.game_state.clone();
    use_effect_with(
        (moves.clone(), eval.clone(), agent_running.clone()),
        move |(moves, eval, agent_running)| {
            if !*agent_running.clone() {
                for mv in moves.iter() {
                    let mut new_state = (*game_state).clone();
                    new_state.apply_move_normalize(mv.clone()).unwrap();
                    let hash = new_state.zobrist_hash;
                    let eval_map = eval.borrow();
                    if !eval_map.contains_key(&hash) {
                        agent_running.set(true);
                        let eval = eval.clone();
                        let agent_running = agent_running.clone();
                        spawn_local(async move {
                            let spec = EvaluationTaskSpec {
                                state: new_state,
                                // this is fast by now, no need to give user choice to prune
                                search_mode: SearchMode::OptimalWL,
                            };
                            let new_evals = eval_task.run(spec).await;
                            eval.borrow_mut().extend(new_evals);
                            agent_running.set(false);
                        });
                        break;
                    }
                }
            }
            || ()
        },
    );

    html! {
        <div class={classes!("player-panel", if is_turn { "active-turn" } else { "" })}>
            <h2>{ format!("Player {}", props.player.id + 1) }</h2>
            <p>{ "Remaining cubies:" }</p>
            <div class="cubies-remaining">
                { for cubies }
            </div>
            <label>
                <input
                    type="checkbox"
                    checked={*move_list_visible}
                    onchange={{
                        let move_list_visible = move_list_visible.clone();
                        move |e: web_sys::Event| {
                            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                            move_list_visible.set(input.checked());
                        }
                    }}
                />
                { "Show move evaluation" }
            </label>
            {
                if *move_list_visible && is_turn {
                    html! {
                        <ul class="move-list">
                            { for moves.iter().map(|mv| {
                                let mut new_state = (*props.game_state).clone();
                                new_state.apply_move_normalize(mv.clone()).unwrap();
                                let eval_map = eval.borrow();
                                let eval = eval_map.get(&new_state.zobrist_hash);
                                let eval = eval_to_string(eval, props.game_state.player_to_move.id);
                                let is_hovered = hovered_move.0.as_ref().map_or(false, |h| h.as_ref() == mv);
                                let mv = mv.clone();
                                html! {
                                    <li
                                        class={if is_hovered { "highlighted" } else { "" }}
                                        style={if is_hovered { format!("--highlight-color: {};", props.player.color) } else { String::new() }}
                                        onclick={apply_move.reform(move |_| mv.clone())}
                                        onmouseenter={ {
                                            let set_hovered_move = set_hovered_move.clone();
                                            let mv = Rc::new(mv.clone());
                                            move |_| set_hovered_move.emit(Some(mv.clone()))
                                        }}
                                        onmouseleave={ {
                                            let set_hovered_move = set_hovered_move.clone();
                                            move |_| set_hovered_move.emit(None)
                                        }}
                                    >
                                        { format!("{}: {}", mv, eval) }
                                    </li>
                                }
                            })}
                        </ul>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}
