use std::{
    io::{self, Write},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};
use tracing_appender::non_blocking::NonBlockingBuilder;

static BLOCK_IN_WORKER: AtomicBool = AtomicBool::new(false);
static BLOCK_DURATION_SECS: AtomicU64 = AtomicU64::new(3);

struct BlockingMemoryWriter {
    buffer: Vec<u8>,
}

impl BlockingMemoryWriter {
    fn new() -> Self {
        Self {
            buffer: Vec::new()
        }
    }
}

impl Write for BlockingMemoryWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if BLOCK_IN_WORKER.load(Ordering::Relaxed) {
            let block_secs = BLOCK_DURATION_SECS.load(Ordering::Relaxed);
            thread::sleep(Duration::from_secs(block_secs));
        }
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_shutdown_timeout_behavior() {
    let block_secs = 2;
    let shutdown_timeout = Duration::from_millis(300);
    BLOCK_DURATION_SECS.store(block_secs, Ordering::Relaxed);

    let blocking_writer = BlockingMemoryWriter::new();
    let (mut non_blocking, guard) = NonBlockingBuilder::default()
        .shutdown_timeout(shutdown_timeout)
        .finish(blocking_writer);

    non_blocking.write_all(b"test data").unwrap();
    thread::sleep(Duration::from_millis(100));

    BLOCK_IN_WORKER.store(true, Ordering::Relaxed);
    non_blocking.write_all(b"blocking data").unwrap();

    let shutdown_start = Instant::now();
    drop(guard);
    let shutdown_duration = shutdown_start.elapsed();

    let expected_min = shutdown_timeout.as_millis() * 9 / 10;
    let expected_max = shutdown_timeout.as_millis() * 11 / 10;

    assert!(
        shutdown_duration.as_millis() > expected_min,
        "Shutdown was too quick: {:?}, expected > {:?}",
        shutdown_duration,
        Duration::from_millis(expected_min as u64)
    );

    assert!(
        shutdown_duration.as_millis() < expected_max,
        "Shutdown took too long: {:?}, expected < {:?}",
        shutdown_duration,
        Duration::from_millis(expected_max as u64)
    );
}

#[test]
fn test_normal_shutdown_without_blocking() {
    let shutdown_timeout = Duration::from_millis(300);
    BLOCK_IN_WORKER.store(false, Ordering::Relaxed);

    let blocking_writer = BlockingMemoryWriter::new();
    let (mut non_blocking, guard) = NonBlockingBuilder::default()
        .shutdown_timeout(shutdown_timeout)
        .finish(blocking_writer);

    non_blocking.write_all(b"test data").unwrap();

    let shutdown_start = Instant::now();
    drop(guard);
    let shutdown_duration = shutdown_start.elapsed();

    assert!(
        shutdown_duration < Duration::from_millis(100),
        "Normal shutdown took too long: {:?}",
        shutdown_duration
    );
}
