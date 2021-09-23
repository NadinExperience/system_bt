
// Copyright (C) 2021 The Android Open Source Project
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
#include "net/posix/posix_async_socket_server.h"

#include <errno.h>       // for errno
#include <netinet/in.h>  // for sockaddr_in, INADDR_ANY
#include <string.h>      // for strerror, NULL
#include <sys/socket.h>  // for accept, bind, getsockname
#include <unistd.h>      // for close
#include <functional>    // for __base, function
#include <type_traits>   // for remove_extent_t

#include "net/posix/posix_async_socket.h"  // for PosixAsyncSocket, AsyncMan...
#include "os/log.h"                        // for LOG_INFO, LOG_ERROR
#include "osi/include/osi.h"               // for OSI_NO_INTR

namespace android {
namespace net {
class AsyncDataChannel;

PosixAsyncSocketServer::PosixAsyncSocketServer(int port, AsyncManager* am)
    : port_(port), am_(am) {
  int listen_fd = 0;
  struct sockaddr_in listen_address {};
  socklen_t sockaddr_in_size = sizeof(struct sockaddr_in);

  OSI_NO_INTR(listen_fd = socket(AF_INET, SOCK_STREAM, 0));
  if (listen_fd < 0) {
    LOG_INFO("Error creating socket for test channel.");
    return;
  }

  int enable = 1;
  if (setsockopt(listen_fd, SOL_SOCKET, SO_REUSEADDR, &enable, sizeof(int)) <
      0) {
    LOG_ERROR("setsockopt(SO_REUSEADDR) failed: %s", strerror(errno));
  }

  listen_address.sin_family = AF_INET;
  listen_address.sin_port = htons(port_);
  listen_address.sin_addr.s_addr = htonl(INADDR_ANY);

  if (bind(listen_fd, reinterpret_cast<sockaddr*>(&listen_address),
           sockaddr_in_size) < 0) {
    LOG_INFO("Error binding test channel listener socket to port: %d, %s", port,
             strerror(errno));
    close(listen_fd);
    return;
  }

  if (listen(listen_fd, 1) < 0) {
    LOG_INFO("Error listening for test channel: %s", strerror(errno));
    close(listen_fd);
    return;
  }

  struct sockaddr_in sin;
  socklen_t slen = sizeof(sin);
  if (getsockname(listen_fd, (struct sockaddr*)&sin, &slen) == -1)
    LOG_INFO("Error retrieving actual port: %s", strerror(errno));
  else
    port_ = ntohs(sin.sin_port);

  LOG_INFO("Listening on: %d (%d)", port_, listen_fd);
  server_socket_ = std::make_shared<PosixAsyncSocket>(listen_fd, am_);
}

bool PosixAsyncSocketServer::StartListening() {
  if (!server_socket_ || !callback_) {
    return false;
  }

  server_socket_->WatchForNonBlockingRead(
      [this](AsyncDataChannel* /* socket */) { AcceptSocket(); });
  return true;
}

void PosixAsyncSocketServer::Close() {
  if (server_socket_) {
    server_socket_->Close();
  }
}

bool PosixAsyncSocketServer::Connected() {
  return server_socket_ && server_socket_->Connected();
}

void PosixAsyncSocketServer::AcceptSocket() {
  int accept_fd = 0;
  OSI_NO_INTR(accept_fd = accept(server_socket_->fd(), NULL, NULL));

  if (accept_fd < 0) {
    LOG_INFO("Error accepting test channel connection errno=%d (%s).", errno,
             strerror(errno));
    return;
  }

  LOG_INFO("accept_fd = %d.", accept_fd);
  StopListening();
  callback_(std::make_shared<PosixAsyncSocket>(accept_fd, am_), this);
}

void PosixAsyncSocketServer::StopListening() { server_socket_->StopWatching(); }
}  // namespace net
}  // namespace android
