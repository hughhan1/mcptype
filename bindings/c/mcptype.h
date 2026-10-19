#ifndef MCPTYPE_H
#define MCPTYPE_H
#include <stddef.h>

typedef struct mcptype_request mcptype_request;
typedef enum { MCPTYPE_OK, MCPTYPE_ERROR, MCPTYPE_CANCELLED } mcptype_status;
typedef struct { mcptype_status status; const char *data; size_t len; } mcptype_response;
mcptype_request *mcptype_request_create(const char *data, size_t len);
void mcptype_request_cancel(const mcptype_request *request);
void mcptype_request_free(mcptype_request *request);
mcptype_response mcptype_compile(const mcptype_request *request);
#endif
