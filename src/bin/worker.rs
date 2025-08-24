//! Evaluates game states during webapp runtime, in a separate thread using a web worker (Yew
//! agent). This avoids blocking the main UI thread.

use rubik_cage::app::agent::EvaluationTask;
use yew_agent::Registrable;

fn main() {
    EvaluationTask::registrar().register();
}
