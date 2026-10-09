# PasteBEAM

> [!WARNING]
> The protocol is not fully finalized yet, so anything can be changed at any moment. For the latest info on how the protocol works always consult this repos code.

TCP-only pastebin-like service.

## Quick Start

### Server

```console
$ cargo run --release -- [<port>] [<posts-root>]
```

By default it listens on port `6969` and stores posts in `./posts/`.

### Server (Docker)

```console
$ docker compose up -d
```

Posts are stored in the `posts` volume.

Without Compose:

```console
$ docker build -t pastebeam .
$ docker run -d --name pastebeam --restart unless-stopped -p 6969:6969 -v pastebeam-posts:/app/posts pastebeam
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
