/*
 * Minimal vendored subset of swupdate_status.h for layout testing.
 * From https://github.com/sbabic/swupdate (LGPL-2.1-or-later)
 */
#pragma once

typedef enum {
	IDLE,
	START,
	RUN,
	SUCCESS,
	FAILURE,
	DOWNLOAD,
	DONE,
	SUBPROCESS,
	PROGRESS
} RECOVERY_STATUS;

typedef enum {
	SOURCE_UNKNOWN,
	SOURCE_WEBSERVER,
	SOURCE_SURICATTA,
	SOURCE_DOWNLOADER,
	SOURCE_LOCAL,
	SOURCE_CHUNKS_DOWNLOADER
} sourcetype;
