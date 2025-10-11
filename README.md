# PasteBEAM

> [!WARNING]
> The protocol is not fully finalized yet, so anything can be changed at any moment. For the latest info on how the protocol works always consult this repos code.

TCP-only pastebin-like service with Proof-of-Work.

## Quick Start

### Server

```console
$ erl
> c(pastebeam).
> pastebeam:start().
```

### Client

#### Get

```
$ telnet <host> <port>
> GET <id>
```

#### Post

```
$ ./post.py <host> <port> <file-path>
```

## Screencast

This project is initially developed on a livestream:

[![thumbnail](./thumbnail.png)](https://www.youtube.com/watch?v=ilH6qb1AP6s)
