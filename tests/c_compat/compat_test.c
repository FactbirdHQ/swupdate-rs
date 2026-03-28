/*
 * C test harness for verifying Rust struct layout compatibility.
 *
 * Compiled by the Rust test via the `cc` crate, then executed.
 * Output is key=value pairs parsed by the Rust test.
 *
 * Commands (argv[1]):
 *   "layout"     - Print sizeof and offsetof for all structs
 *   "progress"   - Serialize a known progress_msg to stdout (binary)
 *   "ipc"        - Serialize a known ipc_message to stdout (binary)
 */

#include "network_ipc.h"
#include "progress_ipc.h"
#include <stdio.h>
#include <stddef.h>
#include <string.h>
#include <stdlib.h>

static void print_layout(void) {
    /* progress_msg */
    printf("sizeof_progress_msg=%zu\n", sizeof(struct progress_msg));
    printf("offsetof_progress_msg_apiversion=%zu\n", offsetof(struct progress_msg, apiversion));
    printf("offsetof_progress_msg_status=%zu\n", offsetof(struct progress_msg, status));
    printf("offsetof_progress_msg_dwl_percent=%zu\n", offsetof(struct progress_msg, dwl_percent));
    printf("offsetof_progress_msg_dwl_bytes=%zu\n", offsetof(struct progress_msg, dwl_bytes));
    printf("offsetof_progress_msg_nsteps=%zu\n", offsetof(struct progress_msg, nsteps));
    printf("offsetof_progress_msg_cur_step=%zu\n", offsetof(struct progress_msg, cur_step));
    printf("offsetof_progress_msg_cur_percent=%zu\n", offsetof(struct progress_msg, cur_percent));
    printf("offsetof_progress_msg_cur_image=%zu\n", offsetof(struct progress_msg, cur_image));
    printf("offsetof_progress_msg_hnd_name=%zu\n", offsetof(struct progress_msg, hnd_name));
    printf("offsetof_progress_msg_source=%zu\n", offsetof(struct progress_msg, source));
    printf("offsetof_progress_msg_infolen=%zu\n", offsetof(struct progress_msg, infolen));
    printf("offsetof_progress_msg_info=%zu\n", offsetof(struct progress_msg, info));

    /* progress_connect_ack */
    printf("sizeof_progress_connect_ack=%zu\n", sizeof(struct progress_connect_ack));

    /* ipc_message */
    printf("sizeof_ipc_message=%zu\n", sizeof(ipc_message));
    printf("sizeof_msgdata=%zu\n", sizeof(msgdata));
    printf("sizeof_swupdate_request=%zu\n", sizeof(struct swupdate_request));

    /* swupdate_request field offsets */
    printf("offsetof_swupdate_request_apiversion=%zu\n", offsetof(struct swupdate_request, apiversion));
    printf("offsetof_swupdate_request_source=%zu\n", offsetof(struct swupdate_request, source));
    printf("offsetof_swupdate_request_dry_run=%zu\n", offsetof(struct swupdate_request, dry_run));
    printf("offsetof_swupdate_request_len=%zu\n", offsetof(struct swupdate_request, len));
    printf("offsetof_swupdate_request_info=%zu\n", offsetof(struct swupdate_request, info));
    printf("offsetof_swupdate_request_software_set=%zu\n", offsetof(struct swupdate_request, software_set));
    printf("offsetof_swupdate_request_running_mode=%zu\n", offsetof(struct swupdate_request, running_mode));
    printf("offsetof_swupdate_request_disable_store_swu=%zu\n", offsetof(struct swupdate_request, disable_store_swu));
}

static void serialize_progress(void) {
    struct progress_msg msg;
    memset(&msg, 0, sizeof(msg));
    msg.apiversion = PROGRESS_API_VERSION;
    msg.status = SUCCESS;    /* 3 */
    msg.dwl_percent = 100;
    msg.dwl_bytes = 1048576; /* 1 MiB */
    msg.nsteps = 2;
    msg.cur_step = 2;
    msg.cur_percent = 100;
    strncpy(msg.cur_image, "rootfs.img", sizeof(msg.cur_image) - 1);
    strncpy(msg.hnd_name, "raw", sizeof(msg.hnd_name) - 1);
    msg.source = SOURCE_LOCAL; /* 4 */
    msg.infolen = 4;
    strncpy(msg.info, "done", sizeof(msg.info) - 1);

    fwrite(&msg, sizeof(msg), 1, stdout);
}

static void serialize_ipc(void) {
    ipc_message msg;
    memset(&msg, 0, sizeof(msg));
    msg.magic = IPC_MAGIC;
    msg.type = REQ_INSTALL;
    msg.data.instmsg.req.apiversion = SWUPDATE_API_VERSION;
    msg.data.instmsg.req.source = SOURCE_LOCAL;
    msg.data.instmsg.req.dry_run = RUN_DEFAULT;
    msg.data.instmsg.req.len = 0;
    strncpy(msg.data.instmsg.req.info, "test firmware", sizeof(msg.data.instmsg.req.info) - 1);
    msg.data.instmsg.len = 0;

    fwrite(&msg, sizeof(msg), 1, stdout);
}

int main(int argc, char *argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: %s <layout|progress|ipc>\n", argv[0]);
        return 1;
    }

    if (strcmp(argv[1], "layout") == 0) {
        print_layout();
    } else if (strcmp(argv[1], "progress") == 0) {
        serialize_progress();
    } else if (strcmp(argv[1], "ipc") == 0) {
        serialize_ipc();
    } else {
        fprintf(stderr, "Unknown command: %s\n", argv[1]);
        return 1;
    }

    return 0;
}
