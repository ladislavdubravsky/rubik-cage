//! Optional computer turns and approximate suggestions, separate from exact evaluations.
use super::{
    agent::{AiTaskSpec, EvaluationTask, EvaluationTaskSpec, TaskStatus},
    evaluation::EvaluationContext,
    utils::move_label,
};
use crate::{
    core::{game::GameState, r#move::Move},
    search::{EvaluationMap, ai::Analysis},
};
use web_sys::HtmlSelectElement;
use yew::{platform::spawn_local, prelude::*};
use yew_agent::oneshot::use_oneshot_runner;

#[derive(Clone, PartialEq)]
struct Board {
    state: GameState,
    history: Vec<GameState>,
}
impl Board {
    fn played(&self, m: Move) -> Option<Self> {
        let mut next = self.clone();
        next.state.apply_move(m).ok()?;
        next.history.push(self.state);
        Some(next)
    }
}

/// Manual interactions notify before changing state, even when a restart/import is a no-op.
#[derive(Clone, PartialEq)]
pub enum AiInteraction {
    Move {
        state: GameState,
        history: Vec<GameState>,
    },
    Reset,
}
#[derive(Clone, PartialEq)]
pub struct AiContext(pub Callback<AiInteraction>);

struct Job {
    session: u64,
    board: Board,
    auto_play: bool,
    started: f64,
    budget: u32,
    batches: u32,
}
struct Controls {
    board: Board,
    expected: Option<Board>,
    generation: u64,
    computer: [bool; 2],
    millis: u32,
    automatic: bool,
    pending: bool,
    job: Option<Job>,
    analysis: Option<Analysis>,
    message: String,
}
impl Controls {
    fn new(board: Board) -> Self {
        Self {
            board,
            expected: None,
            generation: 0,
            computer: [false; 2],
            millis: 1000,
            automatic: false,
            pending: false,
            job: None,
            analysis: None,
            message: String::new(),
        }
    }
    fn cancel(&mut self, pause: bool) {
        self.generation = self.generation.wrapping_add(1);
        self.job = None;
        self.analysis = None;
        self.expected = None;
        if pause {
            self.automatic = false;
        }
        // A cancelled request still occupies the shared worker until its reply arrives.
    }
    fn observe(&mut self, board: Board) {
        if self.board == board {
            return;
        }
        if self.expected.as_ref() == Some(&board) {
            self.expected = None;
        } else if self.expected.as_ref().is_some_and(|next| {
            // The two state handles may render separately; do not start another job midway.
            (board.state == self.board.state || board.state == next.state)
                && (board.history == self.board.history || board.history == next.history)
        }) {
        } else {
            self.cancel(true);
            self.message = "AI paused after the game changed.".into();
        }
        self.board = board;
        if self.board.state.outcome().is_some() {
            self.cancel(true);
            self.message = "Game finished.".into();
        }
    }
    fn interaction(&mut self, action: AiInteraction) {
        match action {
            AiInteraction::Reset => {
                self.cancel(true);
                self.message = "AI paused after a manual change.".into();
            }
            AiInteraction::Move { state, history } => {
                let overriding_computer =
                    self.computer[self.board.state.player_to_move.id as usize];
                self.cancel(overriding_computer);
                self.expected = Some(Board { state, history });
                self.message = if overriding_computer {
                    "AI paused after a manual move.".into()
                } else {
                    String::new()
                };
            }
        }
    }
    fn start(&mut self, auto_play: bool, now: f64) {
        self.cancel(false);
        self.job = Some(Job {
            session: self.generation,
            board: self.board.clone(),
            auto_play,
            started: now,
            budget: self.millis,
            batches: 0,
        });
        self.message = if auto_play {
            "Computer thinking…".into()
        } else {
            "Analyzing suggestion…".into()
        };
    }
    fn accepts(&self, session: u64, board: &Board) -> bool {
        self.generation == session
            && self.board == *board
            && self
                .job
                .as_ref()
                .is_some_and(|job| job.session == session && job.board == *board)
            && board.state.outcome().is_none()
    }
    fn suggested_board(&self) -> Option<Board> {
        let analysis = self.analysis.as_ref()?;
        if analysis.position != self.board.state.position_key() || !analysis.tactical_complete {
            return None;
        }
        self.board.played(analysis.best_move?)
    }
    fn expect_play(&mut self, next: Board) {
        self.cancel(false);
        self.expected = Some(next);
        self.message.clear();
    }
}

fn now() -> f64 {
    web_sys::js_sys::Date::now()
}
fn local_known(eval: &EvaluationContext, state: &GameState) -> EvaluationMap {
    let mut known = EvaluationMap::new();
    if let Some(value) = eval.get(state) {
        known.insert(state.position_key(), value);
    }
    for m in state.legal_moves() {
        let mut child = *state;
        if child.apply_move(m).is_ok() {
            if let Some(value) = eval.get(&child) {
                known.insert(child.position_key(), value);
            }
        }
    }
    known
}

#[derive(Properties, PartialEq)]
pub struct AiControlsProps {
    pub game_state: UseStateHandle<GameState>,
    pub history: UseStateHandle<Vec<GameState>>,
    pub children: Children,
}

#[function_component(AiControls)]
pub fn ai_controls(props: &AiControlsProps) -> Html {
    let board = Board {
        state: *props.game_state,
        history: (*props.history).clone(),
    };
    let controls = use_mut_ref(|| Controls::new(board.clone()));
    let revision = use_state(|| 0u64);
    let runner = use_oneshot_runner::<EvaluationTask>();
    let eval = use_context::<EvaluationContext>().expect("EvaluationProvider");
    // Observe the raw board and history on every render, before a response may apply a move.
    controls.borrow_mut().observe(board.clone());
    let refresh = {
        let revision = revision.clone();
        let sequence = use_mut_ref(|| 0u64);
        Callback::from(move |()| {
            let mut n = sequence.borrow_mut();
            *n = n.wrapping_add(1);
            revision.set(*n);
        })
    };
    {
        let controls = controls.clone();
        let refresh = refresh.clone();
        let game_state = props.game_state.clone();
        let history = props.history.clone();
        use_effect_with((board.clone(), *revision), move |_| {
            let request = {
                let mut c = controls.borrow_mut();
                if c.job.is_none()
                    && c.expected.is_none()
                    && c.automatic
                    && c.computer[c.board.state.player_to_move.id as usize]
                    && c.board.state.outcome().is_none()
                {
                    c.start(true, now());
                }
                if c.pending {
                    None
                } else if let Some(job) = c.job.as_mut() {
                    let request = (job.session, job.board.clone());
                    c.pending = true;
                    Some(request)
                } else {
                    None
                }
            };
            if let Some((session, source)) = request {
                let known = local_known(&eval, &source.state);
                spawn_local(async move {
                    let response = runner
                        .run(EvaluationTaskSpec {
                            state: source.state,
                            request_id: session,
                            batch: 0,
                            ai: Some(AiTaskSpec {
                                session_id: session,
                                max_millis: 40,
                                known,
                            }),
                        })
                        .await;
                    let next = {
                        let mut c = controls.borrow_mut();
                        c.pending = false;
                        if response.request_id != session || !c.accepts(session, &source) {
                            None
                        } else if let TaskStatus::Failed(error) = response.status {
                            c.cancel(true);
                            c.message = format!("AI could not finish: {error}");
                            None
                        } else {
                            if let Some(analysis) = response.ai {
                                if analysis.position == source.state.position_key()
                                    && analysis
                                        .best_move
                                        .is_none_or(|m| source.state.legal_moves().contains(&m))
                                {
                                    c.analysis = Some(analysis);
                                } else {
                                    c.cancel(true);
                                    c.message =
                                        "AI returned an invalid suggestion; analysis stopped."
                                            .into();
                                }
                            }
                            if let Some(job) = c.job.as_mut() {
                                job.batches += 1;
                                let elapsed = (now() - job.started).max(0.0);
                                let budget_over = elapsed >= f64::from(job.budget);
                                // A small extra allowance permits the initial tactical screen or endgame oracle.
                                let hard_stop =
                                    elapsed >= f64::from(job.budget + 1500) || job.batches >= 1000;
                                let auto_play = job.auto_play;
                                let ready =
                                    c.analysis.as_ref().is_some_and(|a| a.tactical_complete);
                                let finished = c.analysis.as_ref().is_some_and(|a| a.finished);
                                if finished || hard_stop || (budget_over && ready) {
                                    c.job = None;
                                    if auto_play && ready {
                                        if let Some(next) = c.suggested_board() {
                                            c.expect_play(next.clone());
                                            Some(next)
                                        } else {
                                            c.automatic = false;
                                            c.message =
                                                "AI paused: no legal suggestion was available."
                                                    .into();
                                            None
                                        }
                                    } else {
                                        if !ready {
                                            c.automatic = false;
                                        }
                                        c.message = if ready {
                                            "Suggestion ready. Exact move evaluations are shown separately.".into()
                                        } else {
                                            "Tactical check incomplete. Try Suggest move with more thinking time.".into()
                                        };
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                    };
                    if let Some(next) = next {
                        history.set(next.history);
                        game_state.set(next.state);
                    }
                    refresh.emit(());
                });
            }
            || ()
        });
    }
    let notice = {
        let controls = controls.clone();
        let refresh = refresh.clone();
        Callback::from(move |action| {
            controls.borrow_mut().interaction(action);
            refresh.emit(());
        })
    };
    let suggest = {
        let controls = controls.clone();
        let refresh = refresh.clone();
        Callback::from(move |_| {
            let mut c = controls.borrow_mut();
            c.automatic = false;
            if c.board.state.outcome().is_none() {
                c.start(false, now());
            }
            drop(c);
            refresh.emit(());
        })
    };
    let play = {
        let controls = controls.clone();
        let refresh = refresh.clone();
        let state = props.game_state.clone();
        let history = props.history.clone();
        Callback::from(move |_| {
            let next = {
                let mut c = controls.borrow_mut();
                let next = c.suggested_board();
                if let Some(next) = &next {
                    c.expect_play(next.clone());
                }
                next
            };
            if let Some(next) = next {
                history.set(next.history);
                state.set(next.state);
            }
            refresh.emit(());
        })
    };
    let start = {
        let controls = controls.clone();
        let refresh = refresh.clone();
        Callback::from(move |_| {
            let mut c = controls.borrow_mut();
            c.cancel(false);
            c.automatic = true;
            c.message = "Automatic computer turns enabled.".into();
            drop(c);
            refresh.emit(());
        })
    };
    let pause = {
        let controls = controls.clone();
        let refresh = refresh.clone();
        Callback::from(move |_| {
            let mut c = controls.borrow_mut();
            c.cancel(true);
            c.message = "AI paused.".into();
            drop(c);
            refresh.emit(());
        })
    };
    let time_change = {
        let controls = controls.clone();
        let refresh = refresh.clone();
        Callback::from(move |event: Event| {
            let input: HtmlSelectElement = event.target_unchecked_into();
            let mut c = controls.borrow_mut();
            c.cancel(true);
            c.millis = input.value().parse().unwrap_or(1000);
            c.message = "Thinking time updated; automatic turns paused.".into();
            drop(c);
            refresh.emit(());
        })
    };
    let c = controls.borrow();
    let terminal = c.board.state.outcome().is_some();
    let status = if terminal {
        "Game finished.".to_owned()
    } else if c.automatic
        && c.job.is_none()
        && !c.computer[c.board.state.player_to_move.id as usize]
    {
        format!(
            "Waiting for Player {}. Automatic computer turns are enabled.",
            c.board.state.player_to_move.id + 1
        )
    } else if c.message.is_empty() {
        "Choose a suggestion or enable computer turns. Manual play is available throughout.".into()
    } else {
        c.message.clone()
    };
    let analysis = c.analysis.as_ref();
    let suggestion = analysis
        .and_then(|a| a.best_move)
        .map(|m| move_label(&c.board.state, m));
    let mut pv_state = c.board.state;
    let line = analysis
        .map(|a| {
            let mut moves = Vec::new();
            for &m in a.principal_variation.iter().take(8) {
                let text = move_label(&pv_state, m);
                if pv_state.apply_move(m).is_err() {
                    break;
                }
                moves.push(text);
            }
            moves.join(" → ")
        })
        .unwrap_or_default();
    html! {
        <ContextProvider<AiContext> context={AiContext(notice)}>
            { for props.children.iter() }
            <section id="ai-controls" class="ai-controls" aria-label="Computer player">
                <h2>{"Computer player"}</h2>
                <div class="ai-options">
                    {for (0..2).map(|id| {
                        let controls = controls.clone(); let refresh = refresh.clone();
                        let computer = c.computer[id];
                        let change = Callback::from(move |event: Event| {
                            let input: HtmlSelectElement = event.target_unchecked_into();
                            let mut c = controls.borrow_mut(); c.cancel(true);
                            c.computer[id] = input.value() == "computer";
                            c.message = "Player settings updated. Start automatic turns when ready.".into();
                            drop(c); refresh.emit(());
                        });
                        html! { <label>{format!("Player {}", id + 1)}
                            <select name={format!("ai-player-{}", id + 1)} onchange={change}>
                                <option value="human" selected={!computer}>{"Human"}</option>
                                <option value="computer" selected={computer}>{"Computer"}</option>
                            </select>
                        </label> }
                    })}
                    <label>{"Thinking time"}<select name="ai-time" onchange={time_change}>
                        <option value="100" selected={c.millis == 100}>{"100 ms"}</option>
                        <option value="1000" selected={c.millis == 1000}>{"1 second"}</option>
                        <option value="10000" selected={c.millis == 10000}>{"10 seconds"}</option>
                    </select></label>
                </div>
                <div class="ai-actions">
                    <button id="ai-suggest" type="button" class="control-button" onclick={suggest} disabled={terminal || c.job.is_some()}>{"Suggest move"}</button>
                    <button id="ai-play" type="button" class="control-button" onclick={play} disabled={terminal || c.suggested_board().is_none()}>{"Play suggested move"}</button>
                    <button id="ai-start" type="button" class="control-button" onclick={start} disabled={terminal || !c.computer.iter().any(|&v| v) || c.automatic}>{"Start automatic turns"}</button>
                    <button id="ai-pause" type="button" class="control-button" onclick={pause} disabled={!c.automatic && c.job.is_none()}>{"Pause AI"}</button>
                </div>
                <p id="ai-status" role="status">{status}</p>
                if let Some(label) = suggestion {
                    <p id="ai-suggestion">{format!("Suggested move: {label}")}</p>
                }
                if let Some(a) = analysis {
                    <p id="ai-details">{format!("Completed depth {} · {} nodes{}", a.completed_depth, a.nodes, if a.tactical_complete { "" } else { " · checking tactics" })}</p>
                    if !line.is_empty() { <p id="ai-line">{format!("Analysis line: {line}")}</p> }
                }
            </section>
        </ContextProvider<AiContext>>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn initial() -> Board {
        Board {
            state: GameState::multicolor(),
            history: Vec::new(),
        }
    }
    #[test]
    fn cancellation_rejects_inflight_results_even_after_identical_restart() {
        let board = initial();
        let mut c = Controls::new(board.clone());
        c.start(true, 0.0);
        let session = c.generation;
        c.pending = true;
        assert!(c.accepts(session, &board));
        c.interaction(AiInteraction::Reset);
        c.observe(board.clone());
        assert!(!c.accepts(session, &board));
        assert!(c.pending);
        assert!(!c.automatic);
        c.start(false, 1.0);
        assert!(!c.accepts(session, &board));
    }
    #[test]
    fn human_turns_continue_but_overrides_and_undo_suspend_computers() {
        let board = initial();
        let mut c = Controls::new(board.clone());
        c.computer = [false, true];
        c.automatic = true;
        let next = board.played(board.state.legal_moves()[0]).unwrap();
        c.interaction(AiInteraction::Move {
            state: next.state,
            history: next.history.clone(),
        });
        c.observe(next.clone());
        assert!(c.automatic);
        let after = next.played(next.state.legal_moves()[0]).unwrap();
        c.interaction(AiInteraction::Move {
            state: after.state,
            history: after.history.clone(),
        });
        c.observe(after);
        assert!(!c.automatic);
        c.automatic = true;
        c.observe(board);
        assert!(!c.automatic);
    }
    #[test]
    fn raw_orientation_and_history_guard_responses_and_expected_ai_moves_continue() {
        let board = initial();
        let mut c = Controls::new(board.clone());
        c.automatic = true;
        c.start(true, 0.0);
        let session = c.generation;
        let mut changed = board.clone();
        changed.history.push(board.state);
        assert!(!c.accepts(session, &changed));
        let next = board.played(board.state.legal_moves()[0]).unwrap();
        c.expect_play(next.clone());
        c.observe(Board {
            state: board.state,
            history: next.history.clone(),
        });
        assert!(c.automatic);
        assert!(c.expected.is_some());
        c.observe(next);
        assert!(c.automatic);
        assert!(c.expected.is_none());
        assert!(!c.accepts(session, &board));

        let left = board
            .played(Move::Drop {
                color: crate::core::cubie::Cubie::White,
                column: (0, 0),
            })
            .unwrap();
        let right = board
            .played(Move::Drop {
                color: crate::core::cubie::Cubie::White,
                column: (2, 2),
            })
            .unwrap();
        assert_eq!(left.state.position_key(), right.state.position_key());
        assert_ne!(left.state, right.state);
        let mut c = Controls::new(left.clone());
        c.start(false, 0.0);
        assert!(c.accepts(c.generation, &left));
        assert!(!c.accepts(c.generation, &right));
    }
}
