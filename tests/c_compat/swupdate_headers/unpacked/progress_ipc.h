/*
 * Minimal vendored subset of progress_ipc.h for layout testing.
 * From https://github.com/sbabic/swupdate (LGPL-2.1-or-later)
 *
 * This is the UNPACKED version matching swupdate <= v2025.05 (before commit
 * 485fd2be added __attribute__((__packed__))).
 */
#pragma once

#include <stdbool.h>
#include "../swupdate_status.h"

#define PRINFOSIZE	2048

#define PROGRESS_API_MAJOR	2
#define PROGRESS_API_MINOR	0
#define PROGRESS_API_PATCH	0

#define PROGRESS_API_VERSION 	((PROGRESS_API_MAJOR & 0xFFFF) << 16 | \
				(PROGRESS_API_MINOR & 0xFF) << 8 | \
				(PROGRESS_API_PATCH & 0xFF))

struct progress_msg {
	unsigned int	apiversion;
	RECOVERY_STATUS	status;
	unsigned int	dwl_percent;
	unsigned long long dwl_bytes;
	unsigned int	nsteps;
	unsigned int	cur_step;
	unsigned int	cur_percent;
	char		cur_image[256];
	char		hnd_name[64];
	sourcetype	source;
	unsigned int 	infolen;
	char		info[PRINFOSIZE];
};

#define PROGRESS_CONNECT_ACK_MAGIC "ACK"
struct progress_connect_ack {
	unsigned int apiversion;
	char magic[4];
};
