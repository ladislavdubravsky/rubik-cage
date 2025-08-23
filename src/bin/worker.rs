use rubik_cage::app::agent::EvaluationTask;
use yew_agent::Registrable;

fn main() {
    EvaluationTask::registrar().register();
}
