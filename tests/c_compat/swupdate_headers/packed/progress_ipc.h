/*
 * Minimal vendored subset of progress_ipc.h for layout testing.
 * From https://github.com/sbabic/swupdate (LGPL-2.1-or-later)
 */
#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "../swupdate_status.h"

#define PRINFOSIZE	2048

#define PROGRESS_API_MAJOR	2
#define PROGRESS_API_MINOR	0
#define PROGRESS_API_PATCH	0

#define PROGRESS_API_VERSION 	((PROGRESS_API_MAJOR & 0xFFFF) << 16 | \
				(PROGRESS_API_MINOR & 0xFF) << 8 | \
				(PROGRESS_API_PATCH & 0xFF))

struct progress_msg {
	uint32_t	apiversion;
	uint32_t	status;
	uint32_t	dwl_percent;
	unsigned long long dwl_bytes;
	uint32_t	nsteps;
	uint32_t	cur_step;
	uint32_t	cur_percent;
	char		cur_image[256];
	char		hnd_name[64];
	uint32_t	source;
	uint32_t 	infolen;
	char		info[PRINFOSIZE];
} __attribute__ ((__packed__));

#define PROGRESS_CONNECT_ACK_MAGIC "ACK"
struct progress_connect_ack {
	uint32_t apiversion;
	char magic[4];
};
