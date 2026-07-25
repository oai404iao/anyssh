FROM alpine:3.22

RUN apk add --no-cache openssh \
    && adduser -D -s /bin/ash anyssh \
    && echo 'anyssh:anyssh-test' | chpasswd \
    && mkdir -p /run/sshd \
    && ssh-keygen -A

COPY sshd_config /etc/ssh/sshd_config

EXPOSE 22

CMD ["/usr/sbin/sshd", "-D", "-e"]
