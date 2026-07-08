use mpi::task;

#[derive(Default)]
struct Store {
    value: u32,
}

#[task(queue_size = 8)]
impl Store {
    #[start]
    fn start(ctx: &mut StoreContext, value: u32) {
        ctx.with_state(|state| {
            state.value = value;
        });
    }

    #[call(reply = u32)]
    fn get(ctx: &mut StoreContext) -> u32 {
        ctx.with_state(|state| state.value)
    }

    #[event(priority)]
    fn stop(ctx: &mut StoreContext) {
        ctx.stop();
    }
}

#[derive(Default)]
struct Client {
    observed: u32,
}

#[task(queue_size = 8, receives(mpi::Response<u32>))]
impl Client {
    #[start]
    fn start(_ctx: &mut ClientContext) {}

    #[event]
    fn fetch(ctx: &mut ClientContext, store: StoreHandle) {
        let observed = store.get(ctx).await.unwrap();
        ctx.with_state(|state| {
            state.observed = observed;
        });
    }

    #[call(reply = u32)]
    fn observed(ctx: &mut ClientContext) -> u32 {
        ctx.with_state(|state| state.observed)
    }

    #[event(priority)]
    fn stop(ctx: &mut ClientContext) {
        ctx.stop();
    }
}

fn main() {
    let (store, store_runtime) = Store::spawn(Store::default(), 42).unwrap();
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client.fetch_blocking(store.clone()).unwrap();
    assert_eq!(client.observed_blocking().unwrap(), 42);

    client.stop_blocking().unwrap();
    store.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    store_runtime.join().unwrap();
}
