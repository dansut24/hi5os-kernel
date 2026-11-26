/* Hi5OS native TLS adapter. BearSSL supplies TLS and X.509; no verification bypass. */
#include "bearssl.h"
extern int hi5_tls_recv(void *, unsigned char *, size_t);
extern int hi5_tls_send(void *, const unsigned char *, size_t);
#include "roots.h"
static br_ssl_client_context client;
static br_x509_minimal_context validator;
static unsigned char buffer[BR_SSL_BUFSIZE_BIDI];
static const uint16_t suites[]={
    BR_TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
    BR_TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
    BR_TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
    BR_TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
    BR_TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
    BR_TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256
};
static void io_init(br_sslio_context *io,void *ctx){
    br_sslio_init(io,&client.eng,hi5_tls_recv,ctx,hi5_tls_send,ctx);
}
static int error_code(void){int e=br_ssl_engine_last_error(&client.eng);return e?e:255;}
int hi5_tls_request(void *ctx,const unsigned char *host,uint64_t epoch,const unsigned char *seed,const unsigned char *request,size_t len){
    br_sslio_context io;
    if(!host||!*host||!seed||epoch<1577836800ULL||epoch>4102444800ULL||!ROOT_COUNT)return 254;
    br_ssl_client_init_full(&client,&validator,ROOTS,ROOT_COUNT);
    br_ssl_engine_set_versions(&client.eng,BR_TLS12,BR_TLS12);
    br_ssl_engine_set_suites(&client.eng,suites,sizeof suites/sizeof suites[0]);
    br_ssl_engine_add_flags(&client.eng,BR_OPT_NO_RENEGOTIATION);
    br_x509_minimal_set_time(&validator,(uint32_t)(epoch/86400+719528),(uint32_t)(epoch%86400));
    br_x509_minimal_set_hash(&validator,br_sha1_ID,0);
    br_x509_minimal_set_hash(&validator,br_md5_ID,0);
    br_x509_minimal_set_minrsa(&validator,256);
    br_ssl_engine_set_buffer(&client.eng,buffer,sizeof buffer,1);
    br_ssl_engine_inject_entropy(&client.eng,seed,48);
    if(!br_ssl_client_reset(&client,(const char*)host,0))return error_code();
    io_init(&io,ctx);
    if(br_sslio_write_all(&io,request,len)<0||br_sslio_flush(&io)<0)return error_code();
    return 0;
}
int hi5_tls_read(void *ctx,unsigned char *out,size_t len){
    br_sslio_context io;io_init(&io,ctx);
    int n=br_sslio_read(&io,out,len);
    if(n>=0)return n;
    if((br_ssl_engine_current_state(&client.eng)&BR_SSL_CLOSED)&&br_ssl_engine_last_error(&client.eng)==0)return 0;
    return -error_code();
}
void hi5_tls_close(void *ctx){br_sslio_context io;io_init(&io,ctx);(void)br_sslio_close(&io);}
