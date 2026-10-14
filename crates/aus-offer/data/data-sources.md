# Get Generic Plans

- The maximum number of records per API call is 1000. 

- The default endpoint returns one page of 25 plans. To access, you must make a separate call for each page. This can be modified by setting the page request parameter to the required page number. Alternatively, setting the page-size request parameter to a maximum of 1000 will enable you to reduce the number of pages (and calls) to retrieve all plans.

You can consult here for exact information at [Energy Product Reference Data](https://www.aer.gov.au/energy-product-reference-data?id=how-can-i-get-access-to-the-energy-made-easy-plan-data&utm_source=startsat60&utm_medium=arcs&utm_campaign=trilogy2021&utm_id=med%3Aoth%3Aclac%3Amp%3A100122%3Asponsoredarticle1.2&page=4)


# Endpoints: 

get all agl offers per page 


 `curl -X GET "https://cdr.energymadeeasy.gov.au/agl/cds-au/v1/energy/plans?fuelType=ELECTRICITY&page-size=1"   -H "x-v: 1"   -H "Accept: application/json"
`


Get 1 offer/plan detail  




` curl -X GET "https://cdr.energymadeeasy.gov.au/agl/cds-au/v1/energy/plans/AGL907700MBE6@EME"   -H "x-v: 3"   -H "Accept: application/json"`



To get the list of retailers is 


`curl -X GET "https://api.cdr.gov.au/cdr-register/v1/all/data-holders/brands/summary"   -H "x-v: 1"   -H "Accept: application/json"`
