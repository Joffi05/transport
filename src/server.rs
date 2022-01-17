use std::error::Error;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::io::{BufReader, AsyncBufReadExt};


async fn start_server<T: ToSocketAddrs>(addr: T) -> Result<(), Box<dyn Error>> {
    //TODO Task Sceduler in this fun, so that this can run in thread
    
    //bind listener
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (socket, adress) = listener.accept().await?;
        
        tokio::spawn(async move {
            //process(socket).await;
        });
    }
}


async fn process(mut socket: TcpStream) {
    let mut socket = BufReader::new(socket);

    let buf: &mut String = &mut String::new();

    socket.read_line(buf).await;
}