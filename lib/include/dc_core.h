#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Element result
 */
typedef enum DcElementResult {
  DcElementResult_Err,
  DcElementResult_Close,
  DcElementResult_Msg,
  DcElementResult_MsgBuf,
} DcElementResult;

enum DcLogLevel
#ifdef __cplusplus
  : uint8_t
#endif // __cplusplus
 {
  DcLogLevel_Error = 0,
  DcLogLevel_Warn = 1,
  DcLogLevel_Info = 2,
  DcLogLevel_Debug = 3,
  DcLogLevel_Trace = 4,
};
#ifndef __cplusplus
typedef uint8_t DcLogLevel;
#endif // __cplusplus

enum DcMetadataType
#ifdef __cplusplus
  : uint8_t
#endif // __cplusplus
 {
  DcMetadataType_Empty = 0,
  DcMetadataType_Int64 = 1,
  DcMetadataType_Float64 = 2,
  DcMetadataType_Duration = 3,
};
#ifndef __cplusplus
typedef uint8_t DcMetadataType;
#endif // __cplusplus

typedef uint32_t DcMetadataId;

/**
 * Reference counted message.
 */
typedef struct DcMsg {
  void *_ptr;
  uintptr_t _size;
} DcMsg;

typedef struct DcDuration {
  uint64_t secs;
  uint32_t nsecs;
} DcDuration;

typedef union DcMetadataValue {
  int64_t int64;
  double float64;
  struct DcDuration duration;
} DcMetadataValue;

typedef struct DcMetadata {
  DcMetadataId id;
  DcMetadataType type;
  union DcMetadataValue value;
} DcMetadata;

/**
 * Message buffer
 */
typedef struct DcMsgBuf DcMsgBuf;

/**
 * Message receiver
 */
typedef struct DcMsgReceiver DcMsgReceiver;

/**
 * Port number
 */
typedef uint8_t DcPort;

/**
 * DcPipeline provides interaction with the runtime context.
 */
typedef struct DcPipeline DcPipeline;

/**
 * Device connector plugin
 */
typedef struct DcPlugin DcPlugin;

/**
 * Device connector element
 */
typedef struct DcElement DcElement;

typedef void *(*DcElementNewFunc)(const char *config);

typedef enum DcElementResult (*DcElementNextFunc)(void *element,
                                                  struct DcPipeline*,
                                                  struct DcMsgReceiver*);

typedef void (*DcElementFreeFunc)(void *element);

typedef bool (*DcFinalizerFunc)(void*);

/**
 * Finalizer for element
 */
typedef struct DcFinalizer {
  DcFinalizerFunc f;
  void *context;
} DcFinalizer;

typedef bool (*DcElementFinalizerCreatorFunc)(void *element, struct DcFinalizer *finalizer);

/**
 * Device connector runner
 */
typedef struct DcRunner DcRunner;

typedef bool (*DcPluginInitFunc)(struct DcPlugin *dc_plugin);

typedef struct DcElementInfo {
  const char *id;
  const char *origin;
  const char *authors;
  const char *description;
  const char *config_doc;
  DcPort recv_ports;
  DcPort send_ports;
  const char *const *const *recv_msg_types;
  const char *const *const *send_msg_types;
  const char *const *metadata_ids;
  uint8_t _extension_fields[0];
} DcElementInfo;

typedef void (*DcRunnerIterElementsFunc)(void*, const struct DcElementInfo*);

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Initialize logger.
 */
void dc_log_init(DcLogLevel level);

/**
 * Get current log level.
 */
DcLogLevel dc_log_get_level(void);

/**
 * Append a log.
 */
void dc_log(DcLogLevel level, const char *plugin, const char *module, const char *msg);

/**
 * Get a metadata id from given string.
 * Return zero if given string is invalid or unknown. If this function is called from out of task threads, returns zero also.
 */
DcMetadataId dc_metadata_get_id(const char *string_id);

/**
 * Clone a DcMsg. Increases the reference counter.
 */
struct DcMsg dc_msg_clone(const struct DcMsg *msg);

/**
 * Free a DcMsg. Decrease the reference counter.
 */
void dc_msg_free(struct DcMsg msg);

/**
 * Get data from a message.
 */
void dc_msg_get_data(const struct DcMsg *msg, const uint8_t **data, uintptr_t *len);

/**
 * Get metadata from a message.
 */
struct DcMetadata dc_msg_get_metadata(const struct DcMsg *msg, DcMetadataId id);

/**
 * Set metadata to a message.
 */
void dc_msg_set_metadata(struct DcMsg *msg, struct DcMetadata metadata);

/**
 * Create a message buffer.
 */
struct DcMsgBuf *dc_msg_buf_new(void);

/**
 * Write data to a message buffer.
 */
void dc_msg_buf_write(struct DcMsgBuf *msg_buf, const uint8_t *data, size_t len);

/**
 * Set metadata to a message buffer.
 */
void dc_msg_buf_set_metadata(struct DcMsgBuf *msg_buf, struct DcMetadata metadata);

/**
 * Take message from a message buffer. Clears the buffer.
 */
struct DcMsg dc_msg_buf_take_msg(struct DcMsgBuf *msg_buf);

/**
 * Get the current bytes length of this buffer.
 */
uintptr_t dc_msg_buf_get_len(const struct DcMsgBuf *msg_buf);

/**
 * Free a message buffer.
 */
void dc_msg_buf_free(struct DcMsgBuf *msg_buf);

/**
 * Receive a message from specified port. Return false if sender task closed or an error occured.
 */
bool dc_msg_receiver_recv(struct DcMsgReceiver *msg_receiver, DcPort port, struct DcMsg *msg);

/**
 * Receive a message. Return false if sender task closed or an error occured.
 */
bool dc_msg_receiver_recv_any_port(struct DcMsgReceiver *msg_receiver,
                                   DcPort *port,
                                   struct DcMsg *msg);

/**
 * Set an error message.
 */
void dc_pipeline_set_err_msg(struct DcPipeline *pipeline, const char *err_msg);

/**
 * Set a message as a result in next() function.
 */
void dc_pipeline_set_result_msg(struct DcPipeline *pipeline, DcPort port, struct DcMsg msg);

/**
 * Get DcMsgBuf for specified port. MUST NOT specify the port that DcMsgBuf already have been gotten.
 */
struct DcMsgBuf *dc_pipeline_get_msg_buf(struct DcPipeline *pipeline,
                                         DcPort port);

/**
 * Get this execution is closing.
 */
bool dc_pipeline_get_closing(const struct DcPipeline *pipeline);

/**
 * Set flag that this execution is closing.
 */
void dc_pipeline_close(struct DcPipeline *pipeline);

/**
 * Get DcMetadataId from string id. Return zero if given string is invalid or unknown.
 */
DcMetadataId dc_pipeline_get_metadata_id(const struct DcPipeline *pipeline, const char *string_id);

extern bool dc_plugin_init(struct DcPlugin *dc_plugin);

/**
 * Set name to this plugin.
 */
bool dc_plugin_set_name(struct DcPlugin *plugin, const char *name);

/**
 * Set framework version to this plugin.
 */
bool dc_plugin_set_version(struct DcPlugin *plugin, const char *version);

/**
 * Register a element to this plugin.
 */
void dc_plugin_register_element(struct DcPlugin *plugin, const struct DcElement *element);

/**
 * Set authors to this plugin.
 */
bool dc_plugin_set_authors(struct DcPlugin *plugin, const char *authors);

/**
 * Create an element.
 */
struct DcElement *dc_element_new(const char *name,
                                 DcPort recv_ports,
                                 DcPort send_ports,
                                 DcElementNewFunc new_,
                                 DcElementNextFunc next,
                                 DcElementFreeFunc free);

/**
 * Set a description to an element.
 */
void dc_element_set_description(struct DcElement *element, const char *desc);

/**
 * Set a configration document to an element.
 */
void dc_element_set_config_doc(struct DcElement *element, const char *config_doc);

/**
 * Set a message type for receiving to an element.
 */
bool dc_element_append_recv_msg_type(struct DcElement *element, DcPort port, const char *msg_type);

/**
 * Set a message type for sending to an element.
 */
bool dc_element_append_send_msg_type(struct DcElement *element, DcPort port, const char *msg_type);

/**
 * Set a metadata id to an element.
 */
bool dc_element_append_metadata_id(struct DcElement *element, const char *metadata_id);

/**
 * Set finalizer creator to an element.
 */
void dc_element_set_finalizer_creator(struct DcElement *element, DcElementFinalizerCreatorFunc f);

/**
 * Create a runner.
 */
struct DcRunner *dc_runner_new(void);

/**
 * Set configuration to a runner.
 */
void dc_runner_set_config(struct DcRunner *runner, const char *config);

/**
 * Append a path to directory that includes plugin files.
 */
void dc_runner_append_dir(struct DcRunner *runner, const char *path);

/**
 * Append a path to a plugin file.
 */
void dc_runner_append_file(struct DcRunner *runner, const char *path);

/**
 * Append a plugin init function.
 */
void dc_runner_append_plugin_init(struct DcRunner *runner, const char *name, DcPluginInitFunc f);

/**
 * Run.
 */
int dc_runner_run(struct DcRunner *runner);

/**
 * Iterate elements by callback.
 */
void dc_runner_iter_elements(struct DcRunner *runner, DcRunnerIterElementsFunc f, void *p);

#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus
