# CinaVault Domain Language

- **Account**: A person's CinaVault identity, used across devices. An account may own or be invited to one or more owned servers.
- **Owned server**: A CinaVault Server installation that stores and serves media controlled by its owner. Media remains on this installation unless streamed to an authorized client.
- **Server claim**: The durable association proving that an account owns an owned server.
- **Server identity**: The stable, non-secret identifier for one CinaVault Server installation.
- **Client**: An installed CinaVault application that discovers and connects to owned servers but does not host a media library.
- **Device registration**: The association between an account and a client installation.
- **Device trust**: Explicit authorization for a registered device to access an owned server.
- **Rendezvous service**: The lightweight central control plane that authenticates accounts, records server claims and device registrations, and returns connection candidates. It does not store or stream media.
- **Connection candidate**: One possible route from a client to an owned server: LAN, public direct, or relay.
- **Direct connect**: A client-to-server connection that does not carry media through the rendezvous service.
- **Relay**: A fallback route for encrypted client-server traffic when direct connection is unavailable.
- **Remote user**: A server-local identity authorized to access an owned server. It is distinct from the cross-device Account until account invitations are implemented.
- **Access key**: A server-issued secret used to authenticate a remote user. Creation and rotation are separate actions.
- **Bearer session**: A short-lived authenticated server session obtained with an access key or password.
- **Opaque media key**: A non-path identifier used by clients to address library items without exposing filesystem paths.
- **Direct play**: Delivery of the stored media bytes without changing the media representation.
- **Direct stream**: Repackaging media without re-encoding its audio or video.
- **Transcode**: Re-encoding media to match client capabilities or bandwidth.
- **Capability negotiation**: Selecting direct play, direct stream, or transcode based on the client, media, network, and server capacity.

