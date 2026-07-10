use std::sync::{Arc, Condvar, Mutex};

use super::ElyShell;

#[derive(Debug)]
struct GateState {
    blockers: usize,
    active: usize,
}

#[derive(Clone, Debug)]
pub(super) struct AuthenticatedOperationGate {
    inner: Arc<(Mutex<GateState>, Condvar)>,
}

#[derive(Debug)]
pub(super) struct AuthenticatedOperationLease {
    inner: Arc<(Mutex<GateState>, Condvar)>,
}

#[derive(Debug)]
pub(super) struct AuthenticatedOperationBarrier {
    inner: Option<Arc<(Mutex<GateState>, Condvar)>>,
}

impl AuthenticatedOperationGate {
    pub(super) fn open() -> Self {
        Self { inner: Arc::new((Mutex::new(GateState { blockers: 0, active: 0 }), Condvar::new())) }
    }

    pub(super) fn try_acquire(&self) -> Option<AuthenticatedOperationLease> {
        let (mutex, _) = self.inner.as_ref();
        let mut state = mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.blockers > 0 {
            return None;
        }
        state.active += 1;
        Some(AuthenticatedOperationLease { inner: self.inner.clone() })
    }

    fn block_new_operations(&self) -> AuthenticatedOperationBarrier {
        let (mutex, _) = self.inner.as_ref();
        let mut state = mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.blockers += 1;
        AuthenticatedOperationBarrier { inner: Some(self.inner.clone()) }
    }
}

impl AuthenticatedOperationBarrier {
    pub(super) fn wait_until_idle(&self) {
        let Some(inner) = &self.inner else {
            return;
        };
        let (mutex, condition) = inner.as_ref();
        let mut state = mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        while state.active > 0 {
            state = condition.wait(state).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    pub(super) fn release(mut self) {
        self.release_inner();
    }

    fn release_inner(&mut self) {
        let Some(inner) = self.inner.take() else {
            return;
        };
        let (mutex, condition) = inner.as_ref();
        let mut state = mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.blockers = state.blockers.saturating_sub(1);
        condition.notify_all();
    }
}

impl Drop for AuthenticatedOperationBarrier {
    fn drop(&mut self) {
        self.release_inner();
    }
}

impl Drop for AuthenticatedOperationLease {
    fn drop(&mut self) {
        let (mutex, condition) = self.inner.as_ref();
        let mut state = mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.active = state.active.saturating_sub(1);
        if state.active == 0 {
            condition.notify_all();
        }
    }
}

impl ElyShell {
    pub(super) fn begin_authenticated_operation(&self) -> Option<AuthenticatedOperationLease> {
        self.authenticated_operation_gate.try_acquire()
    }

    pub(super) fn block_authenticated_operations(&self) -> AuthenticatedOperationBarrier {
        self.authenticated_operation_gate.block_new_operations()
    }

    pub(super) fn hold_auth_flow_barrier(&mut self) {
        let barrier = self.block_authenticated_operations();
        self.auth_flow_barrier = Some(barrier);
    }

    pub(super) fn release_auth_flow_barrier(&mut self) {
        self.auth_flow_barrier.take();
    }
}

#[cfg(test)]
mod tests {
    use super::AuthenticatedOperationGate;

    #[test]
    fn closed_gate_rejects_new_work_and_waits_for_active_work()
    -> Result<(), Box<dyn std::error::Error>> {
        let gate = AuthenticatedOperationGate::open();
        let lease = gate.try_acquire().ok_or("open gate rejected work")?;
        let barrier = gate.block_new_operations();
        assert!(gate.try_acquire().is_none());

        let (sender, receiver) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            barrier.wait_until_idle();
            let _ = sender.send(());
        });
        assert!(receiver.recv_timeout(std::time::Duration::from_millis(20)).is_err());
        drop(lease);
        receiver.recv_timeout(std::time::Duration::from_secs(1))?;
        waiter.join().map_err(|_| "gate waiter panicked")?;
        Ok(())
    }

    #[test]
    fn overlapping_barriers_release_independently() {
        let gate = AuthenticatedOperationGate::open();
        let first = gate.block_new_operations();
        let second = gate.block_new_operations();
        assert!(gate.try_acquire().is_none());

        first.release();
        assert!(gate.try_acquire().is_none());

        second.release();
        assert!(gate.try_acquire().is_some());
    }
}
