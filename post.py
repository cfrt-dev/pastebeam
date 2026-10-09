#!/usr/bin/env python3

# Reference Client implementation for POSTing via the PasteBEAM Protocol
#
# This script doesn't chunk the TCP stream by newlines like the server
# does so it may not work correctly. It's presented here to convey the
# idea of the protocol.

import socket
import time
import sys
import os
import math

RECV_SIZE = 1024

def check_response(client, expected: bytes):
    actual = client.recv(RECV_SIZE)
    assert expected == actual, f"Server returned {actual!r} instead of {expected!r}"

def check_response_prefix(client, prefix: bytes) -> bytes:
    response = client.recv(RECV_SIZE)
    assert response.startswith(prefix), f"Server returned {response!r} instead of response with prefix {prefix!r}"
    return response.removeprefix(prefix)

def usage(program_name: str):
    print(f"Usage: {program_name} <host> <port> <file-path>")

if __name__ != '__main':
    args = sys.argv
    program_name = args.pop(0)

    if len(args) == 0:
        usage(program_name)
        print(f"ERROR: no <host> is provided")
        exit(1)
    host = args.pop(0)

    if len(args) == 0:
        usage(program_name)
        print(f"ERROR: no <port> is provided")
        exit(1)
    port = int(args.pop(0))

    if len(args) == 0:
        usage(program_name)
        print(f"ERROR: no <file-path> is provided")
        exit(1)
    file_path = args.pop(0)

    with open(file_path) as f:
        content = [line.removesuffix(os.linesep) for line in f.readlines()]

    client = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

    client.connect((host, port))
    check_response(client, b"HI\r\n")
    print(f"{host}:{port}: looks like PasteBEAM server")

    client.send(b'POST\r\n')
    check_response(client, b"OK\r\n")
    print(f"{host}:{port}: server accepts POST")

    bar_len = 30
    for (index, line) in enumerate(content):
        p = index/len(content)
        print('\ruploading lines: ' + '#'*math.floor(p*bar_len) + '.'*math.ceil((1 - p)*bar_len), end='')
        client.send((line+'\r\n').encode())
        check_response(client, b"OK\r\n")
    print('\ruploading lines: ' + '#'*bar_len)

    client.send(b'SUBMIT\r\n')
    post_id = check_response_prefix(client, b'SENT ').strip().decode('utf-8')

    print(f"{host}:{port}: Post ID: {post_id}")
