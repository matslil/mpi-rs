use ctx_future::{CtxFuture, CtxPoll, resume_fn};

#[derive(Debug, Eq, PartialEq)]
enum ReserveError {
    NotEnoughCapacity,
}

#[derive(Default)]
struct Pool {
    capacity: usize,
    used: usize,
}

fn main() {
    let mut reserve = resume_fn(|pool: &mut Pool, amount: usize| {
        if pool.used + amount > pool.capacity {
            CtxPoll::Ready(Err(ReserveError::NotEnoughCapacity))
        } else {
            pool.used += amount;
            CtxPoll::Ready(Ok(pool.used))
        }
    });

    let mut pool = Pool {
        capacity: 4,
        used: 0,
    };

    assert_eq!(reserve.resume(&mut pool, 3), CtxPoll::Ready(Ok(3)));
    assert_eq!(
        reserve.resume(&mut pool, 2),
        CtxPoll::Ready(Err(ReserveError::NotEnoughCapacity))
    );
    println!("pool used: {}", pool.used);
}
