use miette::{Context, IntoDiagnostic};
use tokio::io::AsyncWriteExt;

// NOTE: We're using async-compression here, instead of a sync library, purely because reqwest (via
// tower-http) uses it.  Because of how reqwest works internally, we're already depending on a bunch
// of the async infra (tokio, futures-util, etc), so it's pretty much free to add this wrapping

// This is largely taken from https://github.com/seanmonstar/reqwest/blob/master/src/blocking/wait.rs
mod wait {
    use std::{
        error::Error,
        fmt::Display,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
        thread::{self, Thread},
        time::{Duration, Instant},
    };

    pub(crate) fn timeout<F, I, E>(fut: F, timeout: Option<Duration>) -> Result<I, Waited<E>>
    where
        F: Future<Output = Result<I, E>>,
    {
        enter();

        let deadline = timeout.map(|d| Instant::now() + d);

        let thread = ThreadWaker(thread::current());
        // Arc shouldn't be necessary, since `Thread` is reference counted internally,
        // but let's just stay safe for now.
        let waker = Waker::from(Arc::new(thread));
        let mut cx = Context::from_waker(&waker);

        futures_util::pin_mut!(fut);

        loop {
            match fut.as_mut().poll(&mut cx) {
                Poll::Ready(Ok(val)) => return Ok(val),
                Poll::Ready(Err(err)) => return Err(Waited::Inner(err)),
                Poll::Pending => (), // fallthrough
            }

            if let Some(deadline) = deadline {
                let now = Instant::now();
                if now >= deadline {
                    return Err(Waited::TimedOut);
                }

                thread::park_timeout(deadline - now);
            } else {
                thread::park();
            }
        }
    }

    #[derive(Debug)]
    pub(crate) enum Waited<E> {
        TimedOut,
        Inner(E),
    }

    impl<E> Display for Waited<E>
    where
        E: Display,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Waited::TimedOut => write!(f, "Timed out while waiting for future"),
                Waited::Inner(e) => e.fmt(f),
            }
        }
    }

    impl<E> Error for Waited<E> where E: Error {}

    struct ThreadWaker(Thread);

    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }

    fn enter() {
        // Check we aren't already in a runtime
        #[cfg(debug_assertions)]
        {
            let _enter = tokio::runtime::Builder::new_current_thread()
                .build()
                .expect("build shell runtime")
                .enter();
        }
    }
}

macro_rules! impl_decode {
    ($fn: ident, $decoder: ident, $name: literal) => {
        pub(crate) fn $fn(source: &[u8], dest: &mut Vec<u8>) -> miette::Result<()> {
            wait::timeout(
                async {
                    let mut d = async_compression::tokio::write::$decoder::new(dest);
                    d.write_all(source).await?;
                    d.flush().await?;
                    Ok::<_, std::io::Error>(())
                },
                None,
            )
            .into_diagnostic()
            .context(concat!("decompressing ", $name, " stream"))
        }
    };
}

impl_decode!(decode_gzip, GzipDecoder, "gzip");
impl_decode!(decode_zstd, ZstdDecoder, "zstd");
impl_decode!(decode_brotli, BrotliDecoder, "brotli");
impl_decode!(decode_deflate, DeflateDecoder, "deflate");
