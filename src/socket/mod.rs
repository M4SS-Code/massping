use std::{
    io,
    mem::MaybeUninit,
    net::SocketAddr,
    task::{Context, Poll, ready},
};

use tokio::io::unix::AsyncFd;

pub(crate) use self::base::BaseSocket;
use crate::IpVersion;

mod base;

pub(crate) struct Socket {
    fd: AsyncFd<BaseSocket>,
}

impl Socket {
    pub(crate) fn new_icmp<V: IpVersion>() -> io::Result<Self> {
        let base = BaseSocket::new_icmp::<V>()?;

        let fd = AsyncFd::new(base)?;
        Ok(Self { fd })
    }

    pub(crate) fn poll_read(
        &self,
        cx: &mut Context<'_>,
        buf: &mut [MaybeUninit<u8>],
    ) -> Poll<io::Result<(&[u8], SocketAddr)>> {
        loop {
            let mut guard = ready!(self.fd.poll_read_ready(cx))?;

            match guard.try_io(|inner| inner.get_ref().recv(buf)) {
                Ok(Ok((buf, source))) => return Poll::Ready(Ok((buf, source))),
                Ok(Err(err)) => return Poll::Ready(Err(err)),
                Err(_) => continue,
            }
        }
    }

    pub(crate) fn poll_write_to(
        &self,
        cx: &mut Context<'_>,
        buf: &[u8],
        addr: SocketAddr,
    ) -> Poll<io::Result<usize>> {
        loop {
            let mut guard = ready!(self.fd.poll_write_ready(cx))?;

            match guard.try_io(|inner| inner.get_ref().send_to(buf, addr)) {
                Ok(Ok(sent)) => return Poll::Ready(Ok(sent)),
                Ok(Err(err)) => return Poll::Ready(Err(err)),
                Err(_) => continue,
            }
        }
    }
}
