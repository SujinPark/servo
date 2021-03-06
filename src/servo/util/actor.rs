/*!
An actor type
*/

use pipes::{Port, Chan, SharedChan, stream};
use core::task;

/**
The client reference to an actor

Actors are only referred to by opaque handles parameterized
over the actor's message type, which can be considered the
actor's interface.
*/
struct ActorRef<M: Owned> {
    chan: Chan<M>,
}

impl<M: Owned> ActorRef<M> {
    fn send(&self, msg: M) {
        self.chan.send(move msg);
    }
}

/// The local actor interface
trait Actor<M> {
    fn handle(&self, msg: M) -> bool;
}

/// A helper function used by actor constructors
fn spawn<A: Actor<M>, M: Owned>(f: ~fn() -> A) -> ActorRef<M> {
    let (port, chan) = stream();
    do task::spawn |move f, move port| {
        let actor = f();
        loop {
            let msg = port.recv();
            if !actor.handle(move msg) {
                break;
            }
        }
    }

    return ActorRef {
        chan: move chan
    }
}

struct SharedActorRef<M: Owned> {
    chan: SharedChan<M>
}

impl<M: Owned> SharedActorRef<M> {
    fn send(&self, msg: M) {
        self.chan.send(move msg);
    }

    fn clone(&self) -> SharedActorRef<M> {
        SharedActorRef {
            chan: self.chan.clone()
        }
    }
}

fn SharedActorRef<M: Owned>(actor: ActorRef<M>) -> SharedActorRef<M> {
    let chan = match move actor {
        ActorRef {
            chan: move chan
        } => {
            move chan
        }
    };

    SharedActorRef {
        chan: SharedChan(move chan)
    }
}

#[cfg(test)]
mod test {

    enum HelloMsg {
        GetName(Chan<~str>),
        Exit(Chan<()>)
    }

    struct HelloActor {
        name: ~str
    }

    impl HelloActor: Actor<HelloMsg> {
        fn handle(&self, msg: HelloMsg) -> bool {
            match msg {
                GetName(chan) => chan.send(copy self.name),
                Exit(chan) => {
                    chan.send(());
                    return false;
                }
            }

            return true;
        }
    }

    fn HelloActor(name: ~str) -> ActorRef<HelloMsg> {
        do spawn |move name| {
            HelloActor {
                name: copy name
            }
        }        
    }


    #[test]
    fn test_exit() {
        let actor = HelloActor(~"bob");
        let (port, chan) = stream();
        actor.send(Exit(move chan));
        port.recv();
    }

    #[test]
    fn test_shared() {
        let actor = HelloActor(~"bob");
        let actor1 = SharedActorRef(move actor);
        let actor2 = actor1.clone();

        let (port1, chan1) = stream();
        actor1.send(GetName(move chan1));
        let (port2, chan2) = stream();
        actor2.send(GetName(move chan2));

        assert port1.recv() == ~"bob";
        assert port2.recv() == ~"bob";

        let (port, chan) = stream();
        actor1.send(Exit(move chan));
        port.recv();
    }

}
