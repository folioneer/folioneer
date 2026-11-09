//! AGT-100 — a piece of work run as if a given profile were the profile in use: an AI agent's
//! session works in the profile that was in use when the owner allowed it, whatever the window
//! chooses since.

use std::future::Future;

use tokio::task::JoinHandle;

tokio::task_local! {
    static SCOPED_PROFILE: String;
}

/// AGT-100 — runs `work` as if `profile_id` were the profile in use (PFL-012).
pub async fn within_profile<F: Future>(profile_id: String, work: F) -> F::Output {
    SCOPED_PROFILE.scope(profile_id, work).await
}

/// AGT-100 — spawns `work`, carrying the profile it is scoped to, if any, into the new task.
pub fn spawn_within_profile<F>(work: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    match scoped_profile() {
        Some(profile_id) => tokio::spawn(SCOPED_PROFILE.scope(profile_id, work)),
        None => tokio::spawn(work),
    }
}

/// The profile the current work is scoped to, if any.
pub(crate) fn scoped_profile() -> Option<String> {
    SCOPED_PROFILE.try_with(Clone::clone).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // AGT-100 — the scope holds inside the work and a task it spawns, and nowhere else.
    #[tokio::test]
    async fn a_scope_holds_inside_its_work_and_what_it_spawns() {
        assert_eq!(scoped_profile(), None);
        let inner = within_profile("p-1".into(), async {
            let spawned = spawn_within_profile(async { scoped_profile() })
                .await
                .expect("spawned");
            (scoped_profile(), spawned)
        })
        .await;
        assert_eq!(inner, (Some("p-1".into()), Some("p-1".into())));
        assert_eq!(scoped_profile(), None);
    }
}
