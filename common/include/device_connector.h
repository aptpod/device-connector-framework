#pragma once

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Element result value for C
 */
typedef enum DcElementResult {
  DcElementResult_Err,
  DcElementResult_Close,
  DcElementResult_MsgBuf,
} DcElementResult;

/**
 * Dummy type for `Vec<u8>`
 */
typedef struct DcMsgBufInner {

} DcMsgBufInner;

/**
 * Port number of elements.
 */
typedef uint8_t Port;

/**
 * Message buffer
 */
typedef struct DcMsgBuf {
  /**
   * Pointer to MsgBufInner
   */
  struct DcMsgBufInner *inner;
  Port port;
} DcMsgBuf;

typedef union DcMsgInner {
  /**
   * Pointer from Vec<u8>.
   */
  uint8_t *owned;
  /**
   * Pointer to buffer.
   */
  const uint8_t *msg_ref;
} DcMsgInner;

/**
 * Message passing between tasks
 */
typedef struct DcMsg {
  /**
   * Pointer to MsgBufInner
   */
  union DcMsgInner inner;
  uintptr_t len;
  uintptr_t capacity;
  void (*drop)(uint8_t*, uintptr_t, uintptr_t);
} DcMsg;

typedef struct DcMsgReceiverInner {

} DcMsgReceiverInner;

/**
 * Handler for device connector pipeline.
 */
typedef struct DcMsgReceiver {
  /**
   * Pointer to `Box<MsgReceiverInner>`
   */
  struct DcMsgReceiverInner *inner;
  bool (*recv)(struct DcMsgReceiverInner*, Port, struct DcMsg*);
  bool (*recv_any)(struct DcMsgReceiverInner*, Port*, struct DcMsg*);
} DcMsgReceiver;

typedef struct DcMsgTypeInner {

} DcMsgTypeInner;

/**
 * Message type
 */
typedef struct DcMsgType {
  struct DcMsgTypeInner *inner;
} DcMsgType;

typedef struct DcPipelineInner {

} DcPipelineInner;

/**
 * Handler for device connector pipeline.
 */
typedef struct DcPipeline {
  /**
   * Pointer to `Box<PipelineInner>`
   */
  struct DcPipelineInner *inner;
  bool (*send_msg_type_checked)(struct DcPipelineInner*);
  bool (*check_send_msg_type)(struct DcPipelineInner*, Port, struct DcMsgType);
  struct DcMsgBuf *(*msg_buf)(struct DcPipelineInner*);
} DcPipeline;

/**
 * Finalizer for element
 */
typedef struct DcFinalizer {
  bool (*f)(void*);
  void *context;
} DcFinalizer;

/**
 * Device connector element
 */
typedef struct DcElement {
  /**
   * Element Name. Must have static lifetime.
   */
  const char *name;
  /**
   * The number of receiver ports.
   */
  Port recv_ports;
  /**
   * The number of sender ports.
   */
  Port send_ports;
  /**
   * Acceptable MsgType.
   */
  const char *const *acceptable_msg_types;
  /**
   * Config text format passed to new(). Must have static lifetime.
   */
  const char *config_format;
  /**
   * Create new element.
   */
  void *(*new_)(const char *config);
  /**
   * Execute element and returns next value.
   */
  enum DcElementResult (*next)(void *element, struct DcPipeline*, struct DcMsgReceiver*);
  /**
   * Returns element finalizer.
   */
  bool (*finalizer)(void *element, struct DcFinalizer *finalizer);
  /**
   * Free used element.
   */
  void (*free)(void *element);
} DcElement;

/**
 * Device connector plugin
 */
typedef struct DcPlugin {
  const char *version;
  size_t n_element;
  const struct DcElement *elements;
} DcPlugin;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Initialize plugin. Must be called at first in dc_load().
 * # Safety
 * `plugin_name` must points valid null-​terminated string.
 */
void dc_init(const char *plugin_name);

/**
 * # Safety
 * `msg_buf` and `data` must be a valid pointer.
 */
void dc_msg_buf_write(struct DcMsgBuf *msg_buf, const uint8_t *data, size_t len);

/**
 * # Safety
 * `msg` must be a valid pointer.
 */
void dc_msg_free(struct DcMsg msg);

/**
 * # Safety
 * `pipeline` and `msg` must be a valid pointer.
 */
bool dc_msg_receiver_recv(struct DcMsgReceiver *msg_receiver, Port port, struct DcMsg *msg);

/**
 * # Safety
 * `pipeline`, `port` and `msg` must be a valid pointer.
 */
bool dc_msg_receiver_recv_any(struct DcMsgReceiver *msg_receiver, Port *port, struct DcMsg *msg);

/**
 * Returns false if `s` is not valid message type text.
 *
 * # Safety
 * `s` must be a valid pointer.
 */
bool dc_msg_type_new(const char *s, struct DcMsgType *msg_type);

/**
 * # Safety
 * `pipeline` must be a valid pointer.
 */
bool dc_pipeline_send_msg_type_checked(struct DcPipeline *pipeline);

/**
 * # Safety
 * `pipeline` must be a valid pointer.
 */
bool dc_pipeline_check_send_msg_type(struct DcPipeline *pipeline,
                                     uint8_t port,
                                     struct DcMsgType msg_type);

/**
 * # Safety
 * `pipeline` must be a valid pointer.
 */
struct DcMsgBuf *dc_pipeline_msg_buf(struct DcPipeline *pipeline);

#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus
