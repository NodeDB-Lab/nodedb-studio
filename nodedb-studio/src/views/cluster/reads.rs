//! Reads the Cluster tabs share, with what makes each one run again.

use std::future::Future;

use dioxus::prelude::*;

use crate::models::cluster::{ClusterMembers, HostedGroup};
use crate::services::error::StudioError;

/// The groups the selected member hosts, as it sees them.
///
/// The read runs again on a connection switch, after every finished members
/// read (a Refresh or a Retry), and on a new selection. The selected id
/// alone is not enough: a memo notifies only when its value changes, and
/// node 1 of another server, or node 1 after a leader change, has the same
/// id. The old groups would then show under the new server's label.
pub fn use_hosted_groups<F, Fut>(
    conn: Memo<Option<String>>,
    members: Resource<Result<ClusterMembers, StudioError>>,
    selected: Memo<Option<String>>,
    fetch: F,
) -> Resource<Result<Vec<HostedGroup>, StudioError>>
where
    F: Fn(String) -> Fut + 'static,
    Fut: Future<Output = Result<Vec<HostedGroup>, StudioError>> + 'static,
{
    use_resource(move || {
        // Subscribe to both. A finished members read writes the resource
        // even when its value is equal.
        let _conn = conn();
        let _members = members.read().is_some();
        let read = selected().map(&fetch);
        async move {
            match read {
                Some(f) => f.await,
                None => Ok(Vec::new()),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use dioxus_core::NoOpMutations;

    use super::*;
    use crate::models::cluster::Observer;

    type MembersRead = Resource<Result<ClusterMembers, StudioError>>;

    /// Handles the test reaches into the running component with.
    #[derive(Clone, Default)]
    struct Probe {
        /// Times the groups read started.
        reads: Rc<Cell<u32>>,
        members: Rc<RefCell<Option<MembersRead>>>,
        conn: Rc<RefCell<Option<Signal<Option<String>>>>>,
    }

    impl PartialEq for Probe {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }

    /// One member, always node 1: every server answers with the same id.
    fn members() -> ClusterMembers {
        ClusterMembers {
            observed_at: String::new(),
            observer: Observer {
                node_id: "1".into(),
            },
            members: Vec::new(),
        }
    }

    #[component]
    fn Harness(probe: Probe) -> Element {
        let conn_signal = use_signal(|| Some("cluster-a".to_string()));
        let conn = use_memo(move || conn_signal.cloned());
        let members = use_resource(|| async { Ok::<_, StudioError>(members()) });
        let selected = use_memo(move || {
            members
                .read()
                .as_ref()
                .and_then(|r| r.as_ref().ok())
                .map(|m| m.observer.node_id.clone())
        });
        let reads = probe.reads.clone();
        use_hosted_groups(conn, members, selected, move |_id| {
            reads.set(reads.get() + 1);
            async { Ok(Vec::new()) }
        });
        *probe.members.borrow_mut() = Some(members);
        *probe.conn.borrow_mut() = Some(conn_signal);
        rsx! {}
    }

    async fn settle(dom: &mut VirtualDom) {
        for _ in 0..8 {
            tokio::select! {
                _ = dom.wait_for_work() => dom.render_immediate(&mut NoOpMutations),
                _ = tokio::time::sleep(std::time::Duration::from_millis(20)) => return,
            }
        }
    }

    /// A members read that picks the same id must still read the groups
    /// again: after a Refresh the groups are new, and after a switch they
    /// are another server's.
    #[tokio::test]
    async fn every_members_read_and_switch_reads_the_groups_again() {
        let probe = Probe::default();
        let mut dom = VirtualDom::new_with_props(
            Harness,
            HarnessProps {
                probe: probe.clone(),
            },
        );
        dom.rebuild_in_place();
        settle(&mut dom).await;
        assert_eq!(probe.reads.get(), 1, "the first read");

        let members = probe.members.borrow().expect("members resource");
        dom.in_runtime(|| members.clone().restart());
        settle(&mut dom).await;
        assert_eq!(probe.reads.get(), 2, "a Refresh of the members");

        let mut conn = probe.conn.borrow().expect("connection signal");
        dom.in_runtime(|| conn.set(Some("cluster-b".to_string())));
        settle(&mut dom).await;
        assert!(probe.reads.get() >= 3, "a connection switch");
    }
}
