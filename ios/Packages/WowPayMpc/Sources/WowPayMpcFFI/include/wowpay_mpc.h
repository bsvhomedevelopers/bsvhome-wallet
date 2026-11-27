#ifndef WOWPAY_MPC_H
#define WOWPAY_MPC_H

#include <stddef.h>
#include <stdint.h>

typedef struct {
    int32_t status;
    uint64_t handle;
    uint64_t buffer_id;
    const uint8_t *data;
    size_t length;
} WowPayMpcResult;

uint32_t wowpay_mpc_api_version(void);
WowPayMpcResult wowpay_mpc_open(const uint8_t *directory, size_t directory_length,
                               const uint8_t *wallet_id, size_t wallet_id_length,
                               const uint8_t *seal_key, size_t seal_key_length);
WowPayMpcResult wowpay_mpc_exchange(uint64_t handle, const uint8_t *command, size_t command_length);
WowPayMpcResult wowpay_mpc_close(uint64_t handle);
int32_t wowpay_mpc_buffer_free(uint64_t buffer_id);

#endif
