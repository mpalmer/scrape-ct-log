use ct_structs::v1::response::{GetSth as GetSthResponse, ResponseEntry};

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Request {
	Metadata(GetSthResponse),
	Entry(u64, ResponseEntry),
}

pub type Mic = gen_server::Mic<Request, ()>;
