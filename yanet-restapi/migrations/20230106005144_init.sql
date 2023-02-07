create table account (
    account_username                text primary key,
    account_password                text not null
);

create table peer (
    peer_id	                        text primary key,
    peer_password                   text not null,
    peer_accepted                   boolean not null default false
);

create table link_account_peer (
    link_account_username           text not null references account(account_username) on delete cascade,
    link_peer_id	                text not null references peer(peer_id) on delete cascade,
    unique(link_account_username, link_peer_id)
);

create table device (
    device_peer_id	                text not null references peer(peer_id) on delete cascade,
    device_name                     text not null,
    device_data                     jsonb not null default '{}' check (jsonb_typeof(device_data) = 'object'),
    unique(device_peer_id, device_name)
);

create table attribute (
    attribute_peer_id	            text not null references peer(peer_id) on delete cascade,
    attribute_name                  text not null,
    attribute_data                  jsonb not null default 'null',
    attribute_actions               jsonb not null default '{}' check (jsonb_typeof(attribute_actions) = 'object'),
    unique(attribute_peer_id, attribute_name)
);
