/*
 * Minimal vendored subset of network_ipc.h for layout testing.
 * From https://github.com/sbabic/swupdate (LGPL-2.1-or-later)
 */
#pragma once

#include <stdlib.h>
#include <stdbool.h>
#include <string.h>
#include "swupdate_status.h"

#define IPC_MAGIC		0x14052001
#define SWUPDATE_API_VERSION	0x1

typedef enum {
	REQ_INSTALL,
	ACK,
	NACK,
	GET_STATUS,
	POST_UPDATE,
	SWUPDATE_SUBPROCESS,
	SET_AES_KEY,
	SET_UPDATE_STATE,
	GET_UPDATE_STATE,
	REQ_INSTALL_EXT,
	SET_VERSIONS_RANGE,
	NOTIFY_STREAM,
	GET_HW_REVISION,
	SET_SWUPDATE_VARS,
	GET_SWUPDATE_VARS,
	SET_DELTA_URL,
} msgtype;

enum run_type {
	RUN_DEFAULT,
	RUN_DRYRUN,
	RUN_INSTALL
};

struct swupdate_request {
	unsigned int apiversion;
	sourcetype source;
	enum run_type dry_run;
	size_t len;
	char info[512];
	char software_set[256];
	char running_mode[256];
	bool disable_store_swu;
};

typedef union {
	char msg[128];
	struct { int current; int last_result; int error; char desc[2048]; } status;
	struct { int status; int error; int level; char msg[2048]; } notify;
	struct { struct swupdate_request req; unsigned int len; char buf[2048]; } instmsg;
	struct { sourcetype source; int cmd; int timeout; unsigned int len; char buf[2048]; } procmsg;
	struct { char key_ascii[65]; char ivt_ascii[33]; } aeskeymsg;
	struct { char minimum_version[256]; char maximum_version[256]; char current_version[256]; char update_type[256]; } versions;
	struct { char boardname[256]; char revision[256]; } revisions;
	struct { char varnamespace[256]; char varname[256]; char varvalue[256]; } vars;
	struct { char filename[256]; char url[1024]; } dwl_url;
} msgdata;

typedef struct {
	int magic;
	int type;
	msgdata data;
} ipc_message;
