use std::error::Error;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::sync::Semaphore;


async fn start_server<T: ToSocketAddrs>(addr: T) -> Result<(), Box<dyn Error>> {
    //TODO Task Sceduler in this fun, so that this can run in thread

    let listener = TcpListener::bind(addr).await?;
    let sem = Arc::new(Semaphore::new(10));

    loop {
        let (socket, _adress) = listener.accept().await?;
        let sem_clone = Arc::clone(&sem);

        tokio::spawn(async move {
            let aq = sem_clone.try_acquire();
            
            if let Ok(_guard) = aq {
                process(socket).await;
            }
            else {
                panic!("too many connections!");
            }
        });
    }
}

async fn process(mut socket: TcpStream) {
    println!("recvd conn");
}


#[allow(unused)]
#[cfg(test)]
mod tests {

    use std::error::Error;
    use std::os::unix::thread;
    use std::time::Duration;
    use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
    use tokio::io::{BufReader, AsyncBufReadExt};
    use tokio::runtime;

    use super::start_server;


    #[tokio::test]
    async fn test() {
    }
}