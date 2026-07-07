use mpi::task;

#[derive(Default)]
struct QueryServer;

#[task(queue_size = 8)]
impl QueryServer {
    #[start]
    async fn start(&mut self, _ctx: &mut QueryServerContext) {}

    #[stream(item = u32, error = String, batch_size = 2)]
    async fn query(
        &mut self,
        _ctx: &mut QueryServerContext,
        out: &mut mpi::BoxStreamSink<u32, String>,
        count: u32,
    ) -> Result<(), String> {
        for value in 0..count {
            out.push(value).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut QueryServerContext) {
        ctx.stop();
    }
}

#[derive(Default)]
struct QueryClient {
    sum: u32,
}

#[task(queue_size = 8)]
impl QueryClient {
    #[start]
    async fn start(&mut self, _ctx: &mut QueryClientContext) {}

    #[event]
    async fn run_query(&mut self, ctx: &mut QueryClientContext, server: QueryServerHandle) {
        let mut rows = server.query(ctx, 5).unwrap();
        let mut sum = 0;
        while let Some(row) = rows.next(ctx).await.unwrap() {
            sum += row;
        }
        self.sum = sum;
    }

    #[call(reply = u32)]
    async fn sum(&mut self, _ctx: &mut QueryClientContext) -> u32 {
        self.sum
    }

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut QueryClientContext) {
        ctx.stop();
    }
}

fn main() {
    let (server, server_runtime) = QueryServer::spawn(QueryServer).unwrap();
    let (client, client_runtime) = QueryClient::spawn(QueryClient::default()).unwrap();

    client.run_query_blocking(server.clone()).unwrap();
    assert_eq!(client.sum_blocking().unwrap(), 10);

    client.stop_blocking().unwrap();
    server.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    server_runtime.join().unwrap();
}
