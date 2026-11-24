# Operate the Hosted Personal Instance

The personal instance of the author is the first deployment of StoryOS that holds data ([ADR 0104](../adr/0104-run-the-personal-instance-of-the-author-on-the-hosted-topology-with-a-proxy-password-the-model-key-on-the-server-and-a-frozen-baseline.md)). It has one hosted PostgreSQL 17 project and one Ubuntu 24.04 VPS (`x86_64`, 1 GB of memory) with the packaged Server, the packaged Worker, and Caddy. The scripts are in `deploy/personal-instance/`. This document gives the order of the steps. [Operate the Production Web Package](production-web.md) gives the package and the Server profiles.

The repository is public. Do not write the address of the VPS, the Host, the reference of the database project, a connection string, or a secret into a file of the repository, a commit, a pull request, or a comment. Each script takes these values from a parameter or from a file below `/etc/storyos/`.

## The layout on the VPS

| Path | Contents |
| --- | --- |
| `/opt/storyos/releases/<commit>/` | One release package. `<commit>` is the first 12 characters of the source commit. |
| `/opt/storyos/current` | The link to the package in use. Only `install.sh` (the first time) and `upgrade.sh` change it. |
| `/opt/storyos/deploy/` | The copy of `backup.sh` and `lib.sh` that the timer of the backup runs. |
| `/etc/storyos/instance.env` | The parameters of the instance: the Host, the user name of the proxy, the database host, port, and name, the two user names of the pooler, and TLS. No value is a secret. |
| `/etc/storyos/secrets/` | One file for each secret. |
| `/etc/storyos/proxy-auth.caddy` | The user name of the proxy and the hash of its password, which the Caddyfile imports. It belongs to `caddy`. |
| `/etc/storyos/server.env` | The settings of the Server that systemd reads: the runtime URL, the session, and the challenge secret. |
| `/etc/storyos/worker.env` | The env file of the Worker (`STORYOS_ENV_FILE`, ADR 0050): the runtime URL, the model key, and the web search key. It belongs to the user `storyos-worker` of the Worker. The Server runs as the user `storyos`, which cannot read it. |
| `/etc/storyos/user-id` | The ID of the one User. |
| `/var/backups/storyos/` | The daily dumps of the last 14 days. |

Each file of `/etc/storyos/` and each dump has mode 600. The service user `storyos` has no login shell. The units are `storyos-server.service`, `storyos-worker.service`, `storyos-backup.service`, and `storyos-backup.timer`. Their logs go to the journal: `journalctl -u storyos-server`.

The Server listens on `127.0.0.1:3000` in the public HTTPS profile, with `STORYOS_WORKER=0`. The Worker has the real model route (ADR 0104 section 3). Caddy asks for the proxy password on each path, sends HSTS, and redirects port 80 to `https`. It compresses the document, the style sheet, the Web script, and the JSON responses with zstd or gzip. It does not compress the activity stream, so each event goes to the browser when the Server sends it. The URLs of the binaries have `sslmode=require`. `psql`, `pg_dump`, and `pg_restore` of the scripts use `sslmode=verify-full` with `sslrootcert=system`.

## The certificate of the database

The hosted database gives a certificate chain with a private root. Thus the operating system of the VPS must trust that root before the first activation. Do not set `SSL_CERT_FILE` for the Worker: it would remove the roots that the model route needs.

1. Download the root certificate from the database settings page of the provider over HTTPS.
2. On the Mac, write down its SHA-256 fingerprint:

   ```sh
   openssl x509 -in <certificate file> -noout -fingerprint -sha256
   ```

3. `push.sh` copies the file to the VPS as `storyos-deploy/database-ca.crt`. On the VPS, run the same command on that file, and compare the two fingerprints. They must be equal.
4. `install.sh` puts the file into `/usr/local/share/ca-certificates/` and runs `update-ca-certificates`.
5. Check that the certificate chain and the host name verify with the trust store, with the host and the port of `instance.env`. The line `Verify return code: 0 (ok)` must appear:

   ```sh
   openssl s_client -starttls postgres -connect <database host>:<port> -verify_hostname <database host> -verify_return_error < /dev/null
   ```

## A new instance

The steps that have **(author)** need a secret of the author or an account of the author. Only the author does them. An agent gives the commands and does not run them on the VPS.

1. **(author)** Make the database project of the provider in the region nearest to the VPS. Note the host and the port of the session pooler and the project reference. The user names of the pooler are `postgres.<project reference>` and `storyos_runtime.<project reference>`.
2. **(author)** Point the DNS name of the instance to the VPS. Use a DNS record with no CDN proxy ([production-web.md](production-web.md), "Public HTTPS edge").
3. On the Mac, in a clean checkout of the release commit, build the Linux package:

   ```sh
   make release-package-linux
   ```

4. **(author)** Copy the archive, its checksum, the scripts, and the certificate to the VPS:

   ```sh
   deploy/personal-instance/push.sh target/storyos-linux-x86_64-<commit>.tar.gz <user>@<VPS address> <SSH port> <certificate file>
   ```

5. **(author)** On the VPS, run `install.sh` with the command that `push.sh` prints. The first run writes `/etc/storyos/instance.env` from `instance.env.example` and stops. Set each value in that file with `sudo editor /etc/storyos/instance.env`.
6. **(author)** Run `install.sh` again. It installs Caddy and the PostgreSQL 17 client from their package sources, makes the service user and the directories, extracts the package, and writes the units. It stops because the secrets are absent.
7. **(author)** Run `sudo sh storyos-deploy/secrets.sh` in a terminal. It asks for the password of the database admin login, the password of the proxy, the model key, and the web search key, with no echo. It makes the challenge secret, the session token, and the password of the runtime role with `openssl rand`.
8. **(author)** Run `install.sh` a third time. It writes `server.env`, `worker.env`, and the Caddyfile, and Caddy gets the certificate of the Host. When `/etc/caddy/Caddyfile` has an earlier site, the script keeps a copy as `/etc/caddy/Caddyfile.before-storyos`. The scripts do not remove an earlier deployment.
9. **(author)** Run `sudo sh storyos-deploy/activate.sh`. It runs `storyos-storage` of the package with the admin URL until the Storage Activation is `Active`, sets the password of the runtime role, makes the one User row with the admin connection, and starts the two services. The first Create Project needs only the User row.
10. **(author)** Open `https://<Host>` in Google Chrome, and give the user name and the password of the proxy. The page sets the session cookie.

A second run of `install.sh`, `secrets.sh`, or `activate.sh` changes nothing that is correct.

To write one secret again, run `secrets.sh <name>` and then `install.sh`. The names are `database-admin-password`, `proxy-password`, `model-key`, `web-search-key`, `challenge-secret`, `session-token`, and `runtime-password`. For `runtime-password`, run `activate.sh` before `install.sh`, so that the role has the new password before the services restart with it. After `session-token`, remove the cookies of the Host in each browser, because the Server issues no cookie to a request with the old token.

## The session

The session token is the value of the `storyos_session` cookie. The Server issues the cookie on each document GET with the correct Host and no cookie. Thus the proxy password is the only gate in front of the session of the author.

The Server keeps the binding of the session in its memory for 8 hours from the start of the process. A document GET with the matching cookie refreshes the binding. A document GET with no cookie sets the cookie and does not refresh the binding. Thus on a Server that runs for more than 8 hours, the first page load of a browser with no cookie can get `401 authentication_required` on each API call. Load the page a second time. A restart of the Server makes a new binding with the same token, so a cookie stays correct.

## The upgrade

1. On the Mac, build the new Linux package, and run `push.sh` with the new archive.
2. **(author)** On the VPS, run `sudo sh storyos-deploy/upgrade.sh storyos-deploy/<archive>`.
3. **(author)** On the VPS, run the `install.sh` command that `push.sh` prints. `upgrade.sh` does not write the Caddyfile, and `install.sh` writes it from the template of the new scripts and reloads Caddy. When the Caddyfile changes, `install.sh` also restarts the Server and the Worker. When nothing changed, the run changes nothing. `make verify-personal-instance` runs this step on an instance with the Caddyfile that has no compression.

`upgrade.sh` verifies the SHA-256, extracts the package, and checks its Web root. It then stops the Worker and the Server, makes one dump with `backup.sh`, runs `storyos-storage` of the new package, changes the link `/opt/storyos/current`, and starts the two services. When `storyos-storage` fails, the script runs it a second time, because the edge can commit and lose its result: a second run of a committed upgrade answers `Active`, and the script continues with the new package. When the backup fails, or `storyos-storage` fails on the second run too, the script starts the old package again and exits with an error. The earlier package stays in `/opt/storyos/releases/`.

The edge of `storyos-storage` applies the new migrations (ADR 0104 section 4). After the edge, it prints a second line `STORYOS_STORAGE_UPGRADE=from <n> files to <m> files`. After a migration, the old package cannot use the database. To go back to the old package after that, restore the dump of the upgrade into a new database project ("The restore"), and before you start the services, point the link to the old package: `sudo ln -sfn /opt/storyos/releases/<old commit> /opt/storyos/current`.

## The backup

`storyos-backup.timer` starts `/opt/storyos/deploy/backup.sh` each day at 03:00 UTC, and after a start of the VPS when that time is missed. The script makes one `pg_dump` of the schema `storyos` in the custom format with the admin connection. It writes the dump to `/var/backups/storyos/storyos-<UTC time>.dump` with mode 600, and it then removes each dump that is older than 14 days. A failed dump leaves the older dumps and makes the unit fail.

```sh
systemctl list-timers storyos-backup.timer
journalctl -u storyos-backup
sudo /opt/storyos/deploy/backup.sh
```

The dump is a backup of the whole instance, and not Project Restore. The dumps stay only on the VPS, and the hosted project is the copy of the data on another host.

## The restore

`restore.sh` loads one dump into a database of a new database project. The database must have no schema `storyos`, and it must have all or none of the four roles of `0000_roles.sql`. The script does not examine other relations, because the dump loads only the schema `storyos`. It loads: the roles of `0000_roles.sql`, the schema `extensions` with `vector`, the dump, the grants outside the schema `storyos`, and the password of the runtime role. The dump holds the activation proof. Thus the Server of the same package starts with no run of `storyos-storage`.

1. **(author)** Make a new database project. The roles of PostgreSQL are in the cluster, so the database must be in a new project.
2. **(author)** Stop the services: `sudo systemctl stop storyos-worker storyos-server`.
3. **(author)** Set the database values of the new project in `/etc/storyos/instance.env`. Run `secrets.sh database-admin-password` for its admin password.
4. **(author)** Run `sudo sh storyos-deploy/restore.sh /var/backups/storyos/<dump>`. When the load fails, the transaction leaves no table, and a second run into the same database uses the roles of the first run.
5. **(author)** Run `install.sh`, which writes the new runtime URL, and start the services: `sudo systemctl start storyos-server storyos-worker`.

`make check-full` runs the same scripts in the drill `scripts/verify-personal-instance-restore.sh`: it writes one Chapter through the packaged Server, runs `backup.sh`, runs `restore.sh` into a second new database, and reads the Chapter there. `make verify-personal-instance` runs the other scripts in an Ubuntu 24.04 container with the Linux package of the commit. It needs the network, and a fake `systemctl` starts the commands of the units, because a container has no systemd.

## Checklist of the acceptance run

Do these checks on the real instance in the acceptance run (S12-07), and record the result of each one with no secret:

1. The SHA-256 fingerprints of the certificate on the Mac and on the VPS are equal, and the `openssl s_client` check gives `Verify return code: 0 (ok)`.
2. `activate.sh` prints `STORYOS_STORAGE_ACTIVATION=Active` and `The Server answers on 127.0.0.1:3000`.
3. Each file of `/etc/storyos/` has mode 600: `sudo find /etc/storyos -type f ! -perm 600` prints nothing.
4. A request with no password gets `401` from Caddy: `curl -s -o /dev/null -w '%{http_code}' https://<Host>/api/v1/protocol`.
5. `http://<Host>/` redirects to `https`, and a response has the header `Strict-Transport-Security`.
6. In Google Chrome, after the proxy password: the home page opens, a new Project opens, a saved Chapter text stays after a reload, the Project Activity Stream shows a Run of the assistant, and a download of an export completes.
7. A Run uses the real model route, and the Worker log has no refusal of its settings: `journalctl -u storyos-worker`.
8. `sudo /opt/storyos/deploy/backup.sh` writes one dump, and `systemctl list-timers storyos-backup.timer` shows the next run.
9. The earlier deployment is removed, and `/etc/caddy/Caddyfile.before-storyos` is kept or removed by a decision of the author.
