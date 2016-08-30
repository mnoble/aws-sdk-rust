// Copyright 2016 LambdaStack All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Portions borrowed from the rusoto project. See README.md

//! Library Documentation
//!
//! AWS API requests.
//!
//! Wraps the Hyper library to send PUT, POST, DELETE and GET requests.

use std::io::Read;
use std::io::Error as IoError;
use std::error::Error;
use std::fmt;
use std::collections::HashMap;

use hyper::Client;
use hyper::Error as HyperError;
use hyper::header::Headers;
use hyper::method::Method;

use log::LogLevel::Debug;

use aws::common::signature::SignedRequest;

/// Wraps the Hyper Response that comes back from AWS S3.
///
/// All HTTP calls are sent from here.
///
pub struct HttpResponse {
    /// HTTP status code
    pub status: u16,
    /// XML payload
    pub body: String,
    /// Unsorted list of header attributes
    pub headers: HashMap<String, String>,
}

/// HTTP Error returned from the DispatchSignedRequest Trait. It also implements the Error Trait.
#[derive(Debug, PartialEq)]
pub struct HttpDispatchError {
    message: String,
}

impl Error for HttpDispatchError {
    fn description(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for HttpDispatchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<HyperError> for HttpDispatchError {
    fn from(err: HyperError) -> HttpDispatchError {
        HttpDispatchError { message: err.description().to_string() }
    }
}

impl From<IoError> for HttpDispatchError {
    fn from(err: IoError) -> HttpDispatchError {
        HttpDispatchError { message: err.description().to_string() }
    }
}

/// Trait that is added to the Hyper Client type. This is where ALL HTTP calls get sent from.
pub trait DispatchSignedRequest {
    fn dispatch(&self, request: &SignedRequest) -> Result<HttpResponse, HttpDispatchError>;
}

impl DispatchSignedRequest for Client {
    fn dispatch(&self, request: &SignedRequest) -> Result<HttpResponse, HttpDispatchError> {
        let hyper_method = match request.method().as_ref() {
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "DELETE" => Method::Delete,
            "GET" => Method::Get,
            "HEAD" => Method::Head,
            v @ _ => return Err(HttpDispatchError { message: format!("Unsupported HTTP verb {}", v) }),

        };

        // translate the headers map to a format Hyper likes
        let mut hyper_headers = Headers::new();
        for h in request.headers().iter() {
            hyper_headers.set_raw(h.0.to_owned(), h.1.to_owned());
        }

        let mut final_uri = format!("{}://{}{}",
                                    if request.ssl() {
                                        "https"
                                    } else {
                                        "http"
                                    },
                                    request.hostname(),
                                    request.path());
        if !request.canonical_query_string().is_empty() {
            final_uri = final_uri + &format!("?{}", request.canonical_query_string());
        }

        if log_enabled!(Debug) {
            let payload = request.payload().map(|mut payload_bytes| {
                let mut payload_string = String::new();

                payload_bytes.read_to_string(&mut payload_string)
                    .map(|_| payload_string)
                    .unwrap_or_else(|_| String::from("<non-UTF-8 data>"))
            });

            debug!("Full request: \n method: {}\n final_uri: {}\n payload: {:?}\nHeaders:\n",
                   hyper_method,
                   final_uri,
                   payload);
            for h in hyper_headers.iter() {
                debug!("{}:{}", h.name(), h.value_string());
            }
        }

        // SENDS
        let mut hyper_response = match request.payload() {
            None => try!(self.request(hyper_method, &final_uri).headers(hyper_headers).body("").send()),
            Some(payload_contents) => try!(self.request(hyper_method, &final_uri)
                .headers(hyper_headers)
                .body(payload_contents)
                .send()),
        };

        let mut body = String::new();
        try!(hyper_response.read_to_string(&mut body));

        if log_enabled!(Debug) {
            debug!("Response body:\n{}", body);
        }

        let mut headers: HashMap<String, String> = HashMap::new();

        for header in hyper_response.headers.iter() {
            headers.insert(header.name().to_string(), header.value_string());
        }

        Ok(HttpResponse {
            status: hyper_response.status.to_u16(),
            body: body,
            headers: headers,
        })
    }
}
