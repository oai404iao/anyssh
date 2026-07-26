FROM alpine:3.22

RUN apk add --no-cache openssh \
    && adduser -D -s /bin/ash anyssh \
    && echo 'anyssh:anyssh-test' | chpasswd \
    && mkdir -p /run/sshd \
    && rm -f /etc/ssh/ssh_host_*

COPY sshd_config /etc/ssh/sshd_config

EXPOSE 22

CMD ["/bin/sh", "-c", "if [ -n \"${ANYSSH_ALLOW_FROM:-}\" ]; then sed -i \"s/^AllowUsers .*/AllowUsers anyssh@${ANYSSH_ALLOW_FROM}/\" /etc/ssh/sshd_config; fi && ssh-keygen -A >/dev/null 2>&1 && exec /usr/sbin/sshd -D -e"]
